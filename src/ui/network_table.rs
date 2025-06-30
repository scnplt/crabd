use super::common::TableStyle;
use super::common::render_footer;
use crate::event::AppEvent;
use crate::ui::resource_table::ResourceTable;
use crate::ui::resource_table::ResourceTableInfo;
use bollard::secret::Network;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::Stylize;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Style,
    text::Text,
    widgets::{Cell, HighlightSpacing, Row, Table},
};
use regex::Regex;

const ROW_HEIGHT: usize = 3;
const REFRESH_AFTER_TICK: u8 = 10;
const REGEX_NETWORK_IN_USE: &str = r":(?:[^:]+:)?\s*([^\(]+)";
const REGEX_NETWORK_CREATED_AT: &str = r"\.\d+";
const DEFAULT_FOOTER: &str = " <Del/D> remove";

#[derive(Default)]
pub struct NetworkTable {
    style: TableStyle,
    skipped_tick_count_for_refresh: u8,
    info: ResourceTableInfo<NetworkTableRow>,
    err: Option<String>,
}

#[derive(Default)]
pub struct NetworkTableRow {
    id: String,
    name: String,
    driver: String,
    created_at: String,
}

impl ResourceTable for NetworkTable {
    type RowType = NetworkTableRow;

    fn get_table_info(&mut self) -> &mut ResourceTableInfo<Self::RowType> {
        &mut self.info
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = ["ID", "Name", "Driver", "Created At"].into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.info.row_heights.clear();

        let rows = self.info.items.iter().enumerate().map(|(index, network)| {
            let row_style = if index % 2 == 0 { self.style.row_style} else { self.style.alt_row_style };
            let item = network.ref_array();

            if index < self.info.items.len() - 1 {
                self.info.row_heights.push(ROW_HEIGHT);
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

        frame.render_stateful_widget(table, area, &mut self.info.state);
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
}

impl NetworkTable {
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if self.err.is_some() {
            self.err = None;
            return Ok(None);
        }

        let event = match key_event.code {
            KeyCode::Delete | KeyCode::Char('d') => self.get_selected_row()
                .map(|n| AppEvent::RemoveNetwork(n.name.clone())),
            _ => self.handle_nav_key_event(key_event)?,
        };

        Ok(event)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.draw_default(frame, area)
    }

    pub fn tick(&mut self) -> Result<Option<AppEvent>> {
        if self.skipped_tick_count_for_refresh <= REFRESH_AFTER_TICK {
            self.skipped_tick_count_for_refresh += 1;
            return Ok(None);
        }

        self.skipped_tick_count_for_refresh = 0;
        Ok(Some(AppEvent::UpdateNetworks))
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
