use crate::event::AppEvent;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect, widgets::ScrollbarState};

#[derive(Default, Clone)]
pub struct ScrollInfo {
    pub vertical: usize,
    pub horizontal: usize,
    pub vertical_state: ScrollbarState,
    pub horizontal_state: ScrollbarState,
    pub max_vertical: usize,
    pub max_horizontal: usize,
}

pub trait ScrollableInfoBlock {
    type Data;

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>>;

    fn handle_nav_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        let mut event = None;

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => event = Some(AppEvent::Back),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(),
            KeyCode::Right | KeyCode::Char('l') => self.scroll_right(),
            KeyCode::Left | KeyCode::Char('h') => self.scroll_left(),
            KeyCode::Home => self.scroll_to_start(),
            KeyCode::End => self.scroll_to_end(),
            KeyCode::PageUp => self.scroll_to_top(),
            KeyCode::PageDown => self.scroll_to_bottom(),
            _ => {}
        };

        Ok(event)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()>;

    fn tick(&mut self) -> Result<Option<AppEvent>>;

    fn update_data(&mut self, data: Self::Data);

    fn get_scroll_info(&mut self) -> &mut ScrollInfo;

    fn scroll_up(&mut self) {
        let scroll_info = self.get_scroll_info();
        if scroll_info.vertical != 0 {
            scroll_info.vertical -= 1
        }
    }

    fn scroll_down(&mut self) {
        let scroll_info = self.get_scroll_info();
        if scroll_info.vertical != scroll_info.max_vertical {
            scroll_info.vertical += 1;
        }
    }

    fn scroll_right(&mut self) {
        let scroll_info = self.get_scroll_info();
        if scroll_info.horizontal != scroll_info.max_horizontal {
            scroll_info.horizontal += 1;
        }
    }

    fn scroll_left(&mut self) {
        let scroll_info = self.get_scroll_info();
        if scroll_info.horizontal != 0 {
            scroll_info.horizontal -= 1;
        }
    }

    fn scroll_to_start(&mut self) {
        let scroll_info = self.get_scroll_info();
        scroll_info.horizontal = 0;
    }

    fn scroll_to_end(&mut self) {
        let scroll_info = self.get_scroll_info();
        scroll_info.horizontal = scroll_info.max_horizontal;
    }

    fn scroll_to_top(&mut self) {
        let scroll_info = self.get_scroll_info();
        scroll_info.vertical = 0;
    }

    fn scroll_to_bottom(&mut self) {
        let scroll_info = self.get_scroll_info();
        scroll_info.vertical = scroll_info.max_vertical;
    }
}
