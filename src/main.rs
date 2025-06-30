mod app;
mod ui;
mod docker;
mod event;
mod utils;

use crate::app::App;
use color_eyre::eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app = App::new()?;
    let result = app.run(terminal).await;
    ratatui::restore();
    result
}
