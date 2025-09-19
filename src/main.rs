use std::path::PathBuf;
use clap::{Parser, Subcommand};
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
use crate::ui::views::ViewStateRequest;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const FEATURE_VERSION: &str = if cfg!(feature = "sentry") {
    concat!(env!("CARGO_PKG_VERSION"), "+sentry")
} else {
    env!("CARGO_PKG_VERSION")
};

#[derive(Parser, Debug)]
#[command(
    name = "sms-terminal",
    version = FEATURE_VERSION,
    about = "A terminal-based SMS client that can send and receive messages live."
)]
struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub global_args: AppArguments,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Messages {
        #[arg(help = "Phone number to display messages for")]
        phone_number: String,

        #[arg(long, help = "Show messages in reverse order")]
        reversed: bool,

        #[command(flatten)]
        args: AppArguments
    },
    Phonebook {
        #[command(flatten)]
        args: AppArguments
    }
}

#[derive(Parser, Serialize, Deserialize, Debug, Clone)]
struct AppArguments {
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
    pub auth: Option<String>,

    #[cfg(feature = "sentry")]
    #[arg(long, help = "Sentry DSN to use for error reporting")]
    pub sentry: Option<String>
}
impl AppArguments {
    pub fn load_with_file_config(self) -> Self {
        let file_config = Self::load_or_create_file();

        Self {
            theme: self.theme.or(file_config.theme).or(Some(PresetTheme::default())),
            host: self.host.or(file_config.host).or(Some("localhost:3000".to_string())),
            http_uri: self.http_uri.or(file_config.http_uri),
            ws_uri: self.ws_uri.or(file_config.ws_uri),
            ws_enabled: self.ws_enabled.or(file_config.ws_enabled),
            auth: self.auth.or(file_config.auth),

            #[cfg(feature = "sentry")]
            sentry: self.sentry.or(file_config.sentry)
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
impl Default for AppArguments {
    fn default() -> Self {
        Self {
            theme: None,
            host: Some("localhost:3000".to_string()),
            http_uri: None,
            ws_uri: None,
            ws_enabled: Some(false),
            auth: None,

            #[cfg(feature = "sentry")]
            sentry: None
        }
    }
}

/// Contained config representation passed into App.
pub struct TerminalConfig {
    pub client: ClientConfig,
    pub theme: PresetTheme,
    pub websocket: bool,
    pub starting_view: Option<ViewStateRequest>,

    #[cfg(feature = "sentry")]
    pub sentry: Option<String>,
}
impl TerminalConfig {
    pub fn parse() -> Self {
        let cli = Cli::parse();

        // Determine starting view and get the appropriate arguments
        let (starting_view, arguments) = match cli.command {
            Some(Commands::Messages { phone_number, reversed, args }) => (
                Some(ViewStateRequest::Messages { phone_number, reversed }),
                args
            ),
            Some(Commands::Phonebook { args }) => (Some(ViewStateRequest::Phonebook), args),
            None => (None, cli.global_args), // default to phonebook
        };

        // Merge CLI args with file config
        let arguments = arguments.load_with_file_config();
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
            websocket: arguments.ws_enabled.unwrap_or(false),
            starting_view,

            #[cfg(feature = "sentry")]
            sentry: arguments.sentry,
        }
    }
}

#[cfg(feature = "sentry")]
fn init_sentry(dsn: String) -> sentry::ClientInitGuard {
    let panic_integration = sentry_panic::PanicIntegration::default().add_extractor(|_| None);
    sentry::init((dsn, sentry::ClientOptions {
        release: Some(FEATURE_VERSION.into()),
        integrations: vec![std::sync::Arc::new(panic_integration)],
        ..Default::default()
    }))
}

const STARTING_MIN_WIDTH: u16 = 160;
const STARTING_MIN_HEIGHT: u16 = 50;

fn main() -> Result<()> {
    color_eyre::install()?;
    let config = TerminalConfig::parse();

    #[cfg(feature = "sentry")]
    let _sentry_guard = config.sentry.as_ref().map(|dsn| init_sentry(dsn.clone()));

    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {

            let terminal = ratatui::init();
            let should_resize = terminal.size()
                .ok()
                .map(|s| STARTING_MIN_HEIGHT > s.height || STARTING_MIN_WIDTH > s.width)
                .unwrap_or(false);

            if should_resize {
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    crossterm::terminal::SetSize(160, 50)
                );
            }

            // Get the starting view from arguments.
            let starting_view = config.starting_view.as_ref()
                .map(|s| s.clone())
                .unwrap_or_default();

            App::new(config)?.run(terminal, starting_view).await
        });

    ratatui::restore();
    result
}