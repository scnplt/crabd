use bollard::secret::Volume;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Text,
    widgets::{Cell, HighlightSpacing, Row, ScrollbarState, Table, TableState},
};

use crate::{
    components::common::{TableStyle, render_footer, render_scrollbar},
    event::AppEvent,
};

#[derive(Default)]
pub struct VolumeTable {
    state: TableState,
    items: Vec<VolumeTableRow>,
    vertical_state: ScrollbarState,
    style: TableStyle,
    row_heights: Vec<usize>,
    vertical_scroll: usize,
    skipped_tick_count_for_refresh: u8,
}

#[derive(Default)]
pub struct VolumeTableRow {
    name: String,
    driver: String,
    used_by: String,
}

impl VolumeTable {
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        let mut event = None;
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => event = Some(AppEvent::Quit),
            KeyCode::Down | KeyCode::Char('j') => self.next_row(),
            KeyCode::Up | KeyCode::Char('k') => self.previous_row(),
            KeyCode::Delete | KeyCode::Char('d') => {
                // TODO Delete volume
            }
            KeyCode::Enter => {
                // TODO Go to volume details
            }
            _ => {}
        }
        Ok(event)
    }

    fn next_row(&mut self) {
        let index = self
            .state
            .selected()
            .map_or(0, |i| if i >= self.items.len() - 1 { 0 } else { i + 1 });
        self.select_row(index);
    }

    fn previous_row(&mut self) {
        let index = self
            .state
            .selected()
            .map_or(0, |i| if i == 0 { self.items.len() - 1 } else { i - 1 });
        self.select_row(index);
    }

    fn select_row(&mut self, index: usize) {
        self.state.select(Some(index));
        self.vertical_scroll = self.row_heights.iter().take(index).sum();
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        use Constraint::{Length, Min};

        let vertical_layout = Layout::vertical([Min(0), Length(3)]);
        let [content_area, footer_area] = vertical_layout.areas(area);

        let horizontal_content_layout = Layout::horizontal([Min(0), Length(1)]);
        let [table_area, scrollbar_area] = horizontal_content_layout.areas(content_area);

        self.render_table(frame, table_area);

        self.update_scroll_state();

        render_scrollbar(frame, scrollbar_area, &mut self.vertical_state, true);

        let footer_text = " <Ent> details | <Del/D> remove".to_string();
        render_footer(frame, footer_area, footer_text);

        Ok(())
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = ["Name", "Driver", "Used By"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.row_heights.clear();

        let rows = self.items.iter().enumerate().map(|(index, volume)| {
            let row_style = if index % 2 == 0 {
                self.style.row_style
            } else {
                self.style.alt_row_style
            };
            let item = volume.ref_array();
            let height = 3;

            if index < self.items.len() - 1 {
                self.row_heights.push(height);
            }

            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(row_style)
                .height(height as u16)
        });

        let widths = vec![
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Min(0),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .row_highlight_style(self.style.selected_row_style)
            .highlight_symbol(Text::from(vec!["".into(), " ● ".into()]))
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    fn update_scroll_state(&mut self) {}

    pub fn tick(&mut self) -> Result<Option<AppEvent>> {
        if self.skipped_tick_count_for_refresh <= 10 {
            self.skipped_tick_count_for_refresh += 1;
            return Ok(None);
        }

        self.skipped_tick_count_for_refresh = 0;
        Ok(Some(AppEvent::UpdateVolumes))
    }

    pub fn update_with_items(&mut self, items: Vec<VolumeTableRow>) {
        let is_empty_before_update = self.items.is_empty();
        self.items = items;
        if is_empty_before_update && !self.items.is_empty() {
            self.select_row(0);
        }
    }
}

impl VolumeTableRow {
    const fn ref_array(&self) -> [&String; 3] {
        [&self.name, &self.driver, &self.used_by]
    }

    pub fn from_list(volumes: Vec<Volume>) -> Vec<Self> {
        volumes.iter().map(Self::from).collect::<Vec<Self>>()
    }

    fn from(volume: &Volume) -> Self {
        Self {
            name: volume.name.clone(),
            driver: volume.driver.clone(),
            used_by: "".to_string(),
        }
    }
}
