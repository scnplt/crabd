use std::any::Any;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{app::Screen, event::AppEvent};

pub trait Component {
    #[allow(unused_variables)]
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        Ok(None)
    }

    fn tick(&mut self) -> Result<Option<AppEvent>> {
        Ok(None)
    }

    fn is_showing(&self, screen: &Screen) -> bool;

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()>;

    fn as_any_mut(&mut self) -> &mut dyn Any;
}
