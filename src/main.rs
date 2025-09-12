use clap::Parser;
use color_eyre::Result;
use sms_client::config::{ClientConfig, WebsocketConfig};

mod app;
mod error;
mod types;
mod ui;
mod theme;

use app::App;
use crate::theme::PresetTheme;

#[derive(clap::Parser, Debug)]
struct Arguments {

    #[arg(long, value_enum)]
    pub theme: Option<PresetTheme>,

    #[arg(long)]
    pub host: Option<String>,

    #[arg(long)]
    pub http_uri: Option<String>,

    #[arg(long)]
    pub ws_uri: Option<String>,

    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    pub ws_enabled: bool,

    #[arg(long)]
    pub auth: Option<String>
}

pub struct TerminalConfig {
    pub client: ClientConfig,
    pub theme: PresetTheme,
    pub websocket: bool
}
impl TerminalConfig {
    pub fn parse() -> Self {
        let arguments = Arguments::parse();
        let host = arguments.host.unwrap_or_else(|| "localhost:3000".to_string());

        // Create SMS config.
        let mut config = ClientConfig::http_only(
            arguments
                .http_uri
                .unwrap_or_else(|| format!("http://{}", host))
        );
        if arguments.ws_enabled {
            let ws_uri = arguments
                .ws_uri
                .unwrap_or_else(|| format!("ws://{}/ws", host));
            config = config.add_websocket(WebsocketConfig::new(ws_uri));
        }
        if let Some(auth) = arguments.auth {
            config = config.with_auth(auth);
        }

        Self {
            client: config,
            theme: arguments.theme.unwrap_or_default(),
            websocket: arguments.ws_enabled
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let config = TerminalConfig::parse();
    let app = App::new(config)?;

    let terminal = ratatui::init();
    let app_result = app.run(terminal).await;

    ratatui::restore();
    app_result
}