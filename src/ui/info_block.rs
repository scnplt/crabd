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

    /// Returns true when the visible content changed.
    fn update_data(&mut self, data: Self::Data) -> bool;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestBlock {
        scroll: ScrollInfo,
    }

    impl ScrollableInfoBlock for TestBlock {
        type Data = ();

        fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
            self.handle_nav_key_event(key_event)
        }

        fn draw(&mut self, _frame: &mut Frame, _area: Rect) -> Result<()> {
            Ok(())
        }

        fn tick(&mut self) -> Result<Option<AppEvent>> {
            Ok(None)
        }

        fn update_data(&mut self, _data: Self::Data) -> bool {
            false
        }

        fn get_scroll_info(&mut self) -> &mut ScrollInfo {
            &mut self.scroll
        }
    }

    fn block_with_bounds(max_vertical: usize, max_horizontal: usize) -> TestBlock {
        TestBlock {
            scroll: ScrollInfo {
                max_vertical,
                max_horizontal,
                ..Default::default()
            },
        }
    }

    #[test]
    fn scroll_down_stops_at_max_vertical() {
        let mut block = block_with_bounds(2, 0);
        block.scroll_down();
        block.scroll_down();
        block.scroll_down();
        assert_eq!(block.scroll.vertical, 2);
    }

    #[test]
    fn scroll_up_clamps_at_zero() {
        let mut block = block_with_bounds(5, 0);
        block.scroll_up();
        assert_eq!(block.scroll.vertical, 0);
    }

    #[test]
    fn scroll_right_and_left_clamp() {
        let mut block = block_with_bounds(0, 2);
        block.scroll_right();
        block.scroll_right();
        block.scroll_right();
        assert_eq!(block.scroll.horizontal, 2);

        block.scroll_left();
        block.scroll_left();
        block.scroll_left();
        assert_eq!(block.scroll.horizontal, 0);
    }

    #[test]
    fn home_and_end_set_horizontal_bounds() {
        let mut block = block_with_bounds(0, 10);
        block.scroll.horizontal = 5;

        block
            .handle_key_event(KeyEvent::from(KeyCode::End))
            .unwrap();
        assert_eq!(block.scroll.horizontal, 10);

        block
            .handle_key_event(KeyEvent::from(KeyCode::Home))
            .unwrap();
        assert_eq!(block.scroll.horizontal, 0);
    }

    #[test]
    fn page_up_and_page_down_set_vertical_bounds() {
        let mut block = block_with_bounds(10, 0);
        block.scroll.vertical = 5;

        block
            .handle_key_event(KeyEvent::from(KeyCode::PageDown))
            .unwrap();
        assert_eq!(block.scroll.vertical, 10);

        block
            .handle_key_event(KeyEvent::from(KeyCode::PageUp))
            .unwrap();
        assert_eq!(block.scroll.vertical, 0);
    }

    #[test]
    fn esc_and_q_yield_back_event() {
        let mut block = TestBlock::default();

        let event = block
            .handle_key_event(KeyEvent::from(KeyCode::Esc))
            .unwrap();
        assert!(matches!(event, Some(AppEvent::Back)));

        let event = block
            .handle_key_event(KeyEvent::from(KeyCode::Char('q')))
            .unwrap();
        assert!(matches!(event, Some(AppEvent::Back)));
    }
}
