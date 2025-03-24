mod docker;
mod views;

mod app;

use std::io;

use crate::app::App;

fn main() -> io::Result<()>{
    let mut terminal = ratatui::init();
    let app_result = App::default().run(&mut terminal);
    ratatui::restore();
    app_result
}
