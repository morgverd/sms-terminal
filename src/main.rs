use color_eyre::Result;

mod app;
mod error;
mod types;
mod ui;
mod theme;

use app::App;

/// TODO: Add clap parsing so the http/ws api can be easily changed via cmd args.
/// 
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let terminal = ratatui::init();
    let app_result = App::new()?.run(terminal).await;
    ratatui::restore();
    app_result
}