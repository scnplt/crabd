mod app;
mod docker;
mod event;
mod ui;
mod utils;

use crate::app::App;
use color_eyre::eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = match App::new() {
        Ok(app) => app.run(terminal).await,
        Err(e) => Err(e),
    };
    ratatui::restore();
    result
}
