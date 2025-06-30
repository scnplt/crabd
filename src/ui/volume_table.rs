use bollard::secret::Volume;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Rect}, style::{Style, Stylize}, text::Text, widgets::{Cell, HighlightSpacing, Row, Table}, Frame
};
use regex::Regex;

use crate::{
    event::AppEvent, ui::{common::{render_footer, TableStyle}, resource_table::{ResourceTable, ResourceTableInfo}}
};

const ROW_HEIGHT: usize = 3;
const REFRESH_AFTER_TICK: u8 = 10;
const REGEX_VOLUME_IN_USE: &str = r"\[([a-z0-9]+)\]";
const DEFAULT_FOOTER: &str = " <Del/D> remove | <F> force remove";

#[derive(Default)]
pub struct VolumeTable {
    style: TableStyle,
    skipped_tick_count_for_refresh: u8,
    info: ResourceTableInfo<VolumeTableRow>,
    err: Option<String>,
}

#[derive(Default)]
pub struct VolumeTableRow {
    name: String,
    driver: String,
    created_at: String,
}

impl ResourceTable for VolumeTable {
    type RowType = VolumeTableRow;

    fn get_table_info(&mut self) -> &mut ResourceTableInfo<Self::RowType> {
        &mut self.info
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = ["Name", "Driver", "Created At"].into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.info.row_heights.clear();

        let rows = self.info.items.iter().enumerate().map(|(index, volume)| {
            let row_style = if index % 2 == 0 { self.style.row_style } else { self.style.alt_row_style };
            let item = volume.ref_array();

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
            Constraint::Min(30),
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

impl VolumeTable {
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if self.err.is_some() {
            self.err = None;
            return Ok(None);
        }

        let event = match key_event.code {
            KeyCode::Delete | KeyCode::Char('d') => {
                self.get_selected_row().map(|volume| AppEvent::RemoveVolume(volume.name.clone(), false))
            }
            KeyCode::Char('f') => {
                self.get_selected_row().map(|volume| AppEvent::RemoveVolume(volume.name.clone(), true))
            }
            _ => self.handle_nav_key_event(key_event)?
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
        Ok(Some(AppEvent::UpdateVolumes))
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
