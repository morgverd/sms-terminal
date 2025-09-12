use clap::Parser;
use color_eyre::Result;
use sms_client::config::ClientConfig;

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
    pub http: Option<String>,

    #[arg(long)]
    pub auth: Option<String>
}

pub struct TerminalConfig {
    pub client: ClientConfig,
    pub theme: PresetTheme
}
impl TerminalConfig {
    pub fn parse() -> Self {
        let arguments = Arguments::parse();
        let config = ClientConfig::http_only(arguments.http.unwrap_or_else(|| "http://localhost:3000".to_string()));

        Self {
            client: if let Some(auth) = arguments.auth {
                config.with_auth(auth)
            } else {
                config
            },
            theme: arguments.theme.unwrap_or_default()
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