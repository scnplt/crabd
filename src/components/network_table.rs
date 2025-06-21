use super::common::TableStyle;
use super::common::{render_footer, render_scrollbar};
use crate::event::AppEvent;
use bollard::secret::Network;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::Stylize;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Text,
    widgets::{Cell, HighlightSpacing, Row, ScrollbarState, Table, TableState},
};
use regex::Regex;

const ROW_HEIGHT: usize = 3;
const REFRESH_AFTER_TICK: u8 = 10;
const REGEX_NETWORK_IN_USE: &str = r":(?:[^:]+:)?\s*([^\(]+)";
const REGEX_NETWORK_CREATED_AT: &str = r"\.\d+";
const DEFAULT_FOOTER: &str = " <Del/D> remove";

#[derive(Default)]
pub struct NetworkTable {
    state: TableState,
    items: Vec<NetworkTableRow>,
    vertical_state: ScrollbarState,
    style: TableStyle,
    row_heights: Vec<usize>,
    vertical_scroll: usize,
    skipped_tick_count_for_refresh: u8,
    err: Option<String>,
}

#[derive(Default)]
pub struct NetworkTableRow {
    id: String,
    name: String,
    driver: String,
    created_at: String,
}

impl NetworkTable {
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
                if let Some(network) = self.get_selected_network() {
                    event = Some(AppEvent::RemoveNetwork(network.name.clone()))
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

    fn get_selected_network(&self) -> Option<&NetworkTableRow> {
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
        let header = ["ID", "Name", "Driver", "Created At"].into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.row_heights.clear();

        let rows = self.items.iter().enumerate().map(|(index, network)| {
            let row_style = if index % 2 == 0 { self.style.row_style } else { self.style.alt_row_style };
            let item = network.ref_array();

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
            Constraint::Length(15),
            Constraint::Min(15),
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
        Ok(Some(AppEvent::UpdateNetworks))
    }

    pub fn update_with_items(&mut self, items: Vec<NetworkTableRow>) {
        let is_empty_before_update = self.items.is_empty();
        self.items = items;
        if is_empty_before_update && !self.items.is_empty() {
            self.select_row(0);
        }
    }

    pub fn show_remove_network_err(&mut self, err: String) {
        let err_msg = Regex::new(REGEX_NETWORK_IN_USE).ok()
            .and_then(|re| re.captures(&err))
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Something went wrong...".to_string());

        self.err = Some(format!("[ERR] {}", err_msg.trim()))
    }
}

impl NetworkTableRow {
    const fn ref_array(&self) -> [&String; 4] {
        [&self.id, &self.name, &self.driver, &self.created_at]
    }

    pub fn from_list(networks: Vec<Network>) -> Vec<Self> {
        let mut result = networks.iter().map(Self::from).collect::<Vec<Self>>();
        result.sort_by_key(|n| n.name.clone());
        result
    }

    fn from(network: &Network) -> Self {
        let id = format!("{}...", &network.id.as_deref().unwrap_or("-")[..12]);

        let raw_created = network.created.as_deref().unwrap_or_default();
        let re = Regex::new(REGEX_NETWORK_CREATED_AT).unwrap();
        let created_at = re.replace(raw_created, "").to_string();

        Self {
            id,
            name: network.name.as_deref().unwrap_or("-").to_string(),
            driver: network.driver.as_deref().unwrap_or("-").to_string(),
            created_at,
        }
    }
}
