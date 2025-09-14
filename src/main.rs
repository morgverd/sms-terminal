use std::path::PathBuf;
use clap::Parser;
use color_eyre::Result;
use sms_client::config::{ClientConfig, WebsocketConfig};

mod app;
mod error;
mod types;
mod ui;
mod theme;
mod modals;

use app::App;
use serde::{Deserialize, Serialize};
use crate::theme::PresetTheme;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Serialize, Deserialize, Debug, Clone)]
#[command(
    name = "sms-terminal",
    version = VERSION,
    about = "A terminal-based SMS client that can send and receive messages live."
)]
struct Arguments {
    #[arg(long, value_enum, help = "Select a built-in theme to start with")]
    #[serde(default)]
    pub theme: Option<PresetTheme>,

    #[arg(long, help = "Set the server host for HTTP and WebSocket (e.g localhost:3000)")]
    #[serde(default)]
    pub host: Option<String>,

    #[arg(long, help = "Set the HTTP URI, this overrides the host if set (e.g. http://localhost:3000)")]
    #[serde(default)]
    pub http_uri: Option<String>,

    #[arg(long, help = "Set the WebSocket URI, this overrides the host if set (e.g. ws://localhost:3000/ws)")]
    #[serde(default)]
    pub ws_uri: Option<String>,

    #[arg(long, help = "Enable WebSocket support")]
    #[serde(default)]
    pub ws_enabled: Option<bool>,

    #[arg(long, help = "Authorization token to use for HTTP and WebSocket requests")]
    #[serde(default)]
    pub auth: Option<String>
}
impl Arguments {
    pub fn load() -> Self {
        let cli_args = Self::parse();
        let file_config = Self::load_or_create_file();

        Self {
            theme: cli_args.theme.or(file_config.theme).or(Some(PresetTheme::default())),
            host: cli_args.host.or(file_config.host).or(Some("localhost:3000".to_string())),
            http_uri: cli_args.http_uri.or(file_config.http_uri),
            ws_uri: cli_args.ws_uri.or(file_config.ws_uri),
            ws_enabled: cli_args.ws_enabled.or(file_config.ws_enabled),
            auth: cli_args.auth.or(file_config.auth)
        }
    }

    fn load_or_create_file() -> Self {
        let config_path = Self::config_path();
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => {
                    match toml::from_str(&content) {
                        Ok(config) => return config,
                        Err(e) => eprintln!("Failed to parse config: {}, using defaults", e),
                    }
                }
                Err(e) => eprintln!("Failed to read config: {}, using defaults", e),
            }
        }

        // Create default config file if it doesn't exist
        let default_config = Self::default();
        if let Err(e) = default_config.save() {
            eprintln!("Failed to save default config: {}", e);
        }
        default_config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        // Check if local config exists first
        let local = PathBuf::from("sms-terminal-config.toml");
        if local.exists() {
            return local;
        }

        // On Windows, use AppData directory
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return PathBuf::from(appdata).join("sms-terminal").join("config.toml");
            }
        }

        // On Unix, use home directory
        #[cfg(not(windows))]
        {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(".config").join("sms-terminal").join("config.toml");
            }
        }

        // Final fallback to local directory
        local
    }
}
impl Default for Arguments {
    fn default() -> Self {
        Self {
            theme: None,
            host: Some("localhost:3000".to_string()),
            http_uri: None,
            ws_uri: None,
            ws_enabled: Some(false),
            auth: None
        }
    }
}

/// Contained config representation passed into App.
pub struct TerminalConfig {
    pub client: ClientConfig,
    pub theme: PresetTheme,
    pub websocket: bool
}
impl TerminalConfig {
    pub fn parse() -> Self {
        let arguments = Arguments::load();
        let host = arguments.host.unwrap_or_else(|| "localhost:3000".to_string());

        // Create SMS config
        let mut client_config = ClientConfig::http_only(
            arguments.http_uri.unwrap_or_else(|| format!("http://{}", host))
        );
        if let Some(ws_enabled) = arguments.ws_enabled && ws_enabled {
            let ws_uri = arguments.ws_uri.unwrap_or_else(|| format!("ws://{}/ws", host));
            client_config = client_config.add_websocket(WebsocketConfig::new(ws_uri));
        }
        if let Some(auth) = arguments.auth {
            client_config = client_config.with_auth(auth);
        }

        Self {
            client: client_config,
            theme: arguments.theme.unwrap_or_default(),
            websocket: arguments.ws_enabled.unwrap_or(false)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {

    color_eyre::install()?;
    let config = TerminalConfig::parse();
    let app = App::new(config)?;

    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::SetSize(160, 50)
    );

    let terminal = ratatui::init();
    let app_result = app.run(terminal).await;

    ratatui::restore();
    app_result
}