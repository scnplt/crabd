use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{ScrollbarState, TableState},
};

use crate::{event::AppEvent, ui::common::render_scrollbar};

pub struct ResourceTableInfo<RowType> {
    pub items: Vec<RowType>,
    pub state: TableState,
    scrollbar_state: ScrollbarState,
    scroll: usize,
    pub row_heights: Vec<usize>,
}

impl<RowType> Default for ResourceTableInfo<RowType> {
    fn default() -> Self {
        Self {
            items: vec![],
            state: TableState::default().with_selected(0),
            scrollbar_state: ScrollbarState::default(),
            scroll: 0,
            row_heights: vec![],
        }
    }
}

pub trait ResourceTable {
    type RowType;

    fn get_table_info(&mut self) -> &mut ResourceTableInfo<Self::RowType>;

    fn handle_nav_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        let mut event = None;

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => event = Some(AppEvent::Quit),
            KeyCode::Down | KeyCode::Char('j') => self.next_row(),
            KeyCode::Up | KeyCode::Char('k') => self.previous_row(),
            _ => {}
        }

        Ok(event)
    }

    fn next_row(&mut self) {
        let table_info = self.get_table_info();
        let last_index = table_info.items.len() - 1;
        let next_index = table_info.state.selected().map_or(0, |i| if i >= last_index { 0 } else { i + 1 });
        self.select_row(next_index);
    }

    fn previous_row(&mut self) {
        let table_info = self.get_table_info();
        let last_index = table_info.items.len() - 1;
        let previous_index = table_info.state.selected().map_or(0, |i| if i == 0 { last_index } else { i - 1 });
        self.select_row(previous_index);
    }

    fn get_selected_row(&mut self) -> Option<&Self::RowType> {
        let table_info = self.get_table_info();
        table_info.state.selected().and_then(|index| table_info.items.get(index))
    }

    fn select_row(&mut self, index: usize) {
        let table_info = self.get_table_info();
        table_info.state.select(Some(index));
        table_info.scroll = table_info.row_heights.iter().take(index).sum();
    }

    fn draw_default(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        use Constraint::{Length, Min};

        let vertical_layout = Layout::vertical([Min(0), Length(3)]);
        let [content_area, footer_area] = vertical_layout.areas(area);

        let horizontal_content_layout = Layout::horizontal([Min(0), Length(1)]);
        let [table_area, scrollbar_area] = horizontal_content_layout.areas(content_area);

        self.render_table(frame, table_area);

        self.update_scroll_state();

        let table_info = self.get_table_info();
        render_scrollbar(frame, scrollbar_area, &mut table_info.scrollbar_state, true);

        self.render_footer(frame, footer_area);

        Ok(())
    }

    fn update_scroll_state(&mut self) {
        let table_info = self.get_table_info();
        let content_height: usize = table_info.row_heights.iter().sum();
        table_info.scrollbar_state = table_info.scrollbar_state
            .content_length(content_height)
            .position(table_info.scroll);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect);

    #[allow(unused_variables)]
    fn render_footer(&mut self, frame: &mut Frame, area: Rect) {}

    fn update_with_items(&mut self, items: Vec<Self::RowType>) {
        let table_info = self.get_table_info();
        let is_empty_before_update = table_info.items.is_empty();
        table_info.items = items;

        if is_empty_before_update && !table_info.items.is_empty() {
            self.select_row(0);
        }
    }
}
