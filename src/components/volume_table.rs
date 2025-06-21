use bollard::secret::Volume;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect}, style::{Style, Stylize}, text::Text, widgets::{Cell, HighlightSpacing, Row, ScrollbarState, Table, TableState}, Frame
};
use regex::Regex;

use crate::{
    components::common::{TableStyle, render_footer, render_scrollbar},
    event::AppEvent,
};

const ROW_HEIGHT: usize = 3;
const REFRESH_AFTER_TICK: u8 = 10;
const REGEX_VOLUME_IN_USE: &str = r"\[([a-z0-9]+)\]";
const DEFAULT_FOOTER: &str = " <Del/D> remove | <F> force remove";

#[derive(Default)]
pub struct VolumeTable {
    state: TableState,
    items: Vec<VolumeTableRow>,
    vertical_state: ScrollbarState,
    style: TableStyle,
    row_heights: Vec<usize>,
    vertical_scroll: usize,
    skipped_tick_count_for_refresh: u8,
    err: Option<String>,
}

#[derive(Default)]
pub struct VolumeTableRow {
    name: String,
    driver: String,
    created_at: String,
}

impl VolumeTable {
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if self.err.is_some() {
            self.err = None;
            return Ok(None);
        }

        let mut event = None;
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => event = Some(AppEvent::Quit),
            KeyCode::Down | KeyCode::Char('j') => self.next_row(),
            KeyCode::Up | KeyCode::Char('k') => self.previous_row(),
            KeyCode::Delete | KeyCode::Char('d') => {
                if let Some(volume) = self.get_selected_volume() {
                    event = Some(AppEvent::RemoveVolume(volume.name.clone(), false))
                }
            }
            KeyCode::Char('f') => {
                if let Some(volume) = self.get_selected_volume() {
                    event = Some(AppEvent::RemoveVolume(volume.name.clone(), true))
                }
            }
            _ => {}
        }
        Ok(event)
    }

    fn next_row(&mut self) {
        let index = self.state.selected().map_or(0, |i| if i >= self.items.len() - 1 { 0 } else { i + 1 });
        self.select_row(index);
    }

    fn previous_row(&mut self) {
        let index = self.state.selected().map_or(0, |i| if i == 0 { self.items.len() - 1 } else { i - 1 });
        self.select_row(index);
    }

    fn select_row(&mut self, index: usize) {
        self.state.select(Some(index));
        self.vertical_scroll = self.row_heights.iter().take(index).sum();
    }

    fn get_selected_volume(&self) -> Option<&VolumeTableRow> {
        self.state.selected().and_then(|index| self.items.get(index))
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

        self.render_footer(frame, footer_area);

        Ok(())
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = ["Name", "Driver", "Created At"].into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.row_heights.clear();

        let rows = self.items.iter().enumerate().map(|(index, volume)| {
            let row_style = if index % 2 == 0 { self.style.row_style } else { self.style.alt_row_style };
            let item = volume.ref_array();

            if index < self.items.len() - 1 {
                self.row_heights.push(ROW_HEIGHT);
            }

            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(row_style)
                .height(ROW_HEIGHT as u16)
        });

        let widths = vec![
            Constraint::Min(30),
            Constraint::Min(0),
            Constraint::Length(27),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .row_highlight_style(self.style.selected_row_style)
            .highlight_symbol(Text::from(vec!["".into(), " ● ".into()]))
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    fn render_footer(&mut self, frame: &mut Frame, area: Rect) {
        let mut border_style = None;
        let mut footer_text = DEFAULT_FOOTER.to_string();

        if let Some(err) = &self.err {
            border_style = Some(Style::new().red());
            footer_text = err.clone();
        }

        render_footer(frame, area, footer_text, border_style);
    }

    fn update_scroll_state(&mut self) {
        let content_height = self.row_heights.iter().sum::<usize>();
        self.vertical_state = self.vertical_state
            .content_length(content_height)
            .position(self.vertical_scroll);
    }

    pub fn tick(&mut self) -> Result<Option<AppEvent>> {
        if self.skipped_tick_count_for_refresh <= REFRESH_AFTER_TICK {
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

    pub fn show_remove_volume_err(&mut self, err: String) {
        let err_msg = Regex::new(REGEX_VOLUME_IN_USE).ok()
            .and_then(|re| re.captures(&err))
            .and_then(|caps| caps.get(1))
            .map(|m| format!("Volume is in use by container: {}...", &m.as_str()[..15]))
            .unwrap_or_else(|| "Something went wrong...".to_string());

        self.err = Some(format!("[ERR] {}", err_msg))
    }
}

impl VolumeTableRow {
    const fn ref_array(&self) -> [&String; 3] {
        [&self.name, &self.driver, &self.created_at]
    }

    pub fn from_list(volumes: Vec<Volume>) -> Vec<Self> {
        let mut result = volumes.iter().map(Self::from).collect::<Vec<Self>>();
        result.sort_by_key(|v| v.name.clone());
        result
    }

    fn from(volume: &Volume) -> Self {
        Self {
            name: volume.name.clone(),
            driver: volume.driver.clone(),
            created_at: volume.created_at.as_deref().unwrap_or_default().to_string(),
        }
    }
}
