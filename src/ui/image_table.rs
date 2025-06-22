use bollard::secret::ImageSummary;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Style, Stylize},
    text::Text,
    widgets::{Cell, HighlightSpacing, Row, Table},
};

use crate::{
    event::AppEvent,
    ui::{
        common::{TableStyle, render_footer, time_ago_string},
        resource_table::{ResourceTable, ResourceTableInfo},
    },
};

use regex::Regex;

const REFRESH_AFTER_TICK: u8 = 10;
const REGEX_DELETE_IMG_ERR: &str = r"\((?:cannot|must) be forced\) - image is being used by (?:running|stopped) container \w+";
const DEFAULT_FOOTER: &str = " <Del/D> remove | <F> force remove";

#[derive(Default)]
pub struct ImageTable {
    style: TableStyle,
    skipped_tick_count_for_refresh: u8,
    info: ResourceTableInfo<ImageTableRow>,
    err: Option<String>,
}

#[derive(Default)]
pub struct ImageTableRow {
    id: String,
    tags: String,
    size: String,
    created: String,
}

impl ResourceTable for ImageTable {
    type RowType = ImageTableRow;

    fn get_table_info(&mut self) -> &mut ResourceTableInfo<Self::RowType> {
        &mut self.info
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = ["ID", "Tags", "Size", "Created"].into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.info.row_heights.clear();

        let rows = self.info.items.iter().enumerate().map(|(index, image)| {
            let row_style = if index % 2 == 0 { self.style.row_style } else { self.style.alt_row_style };
            let item = image.ref_array();
            let tags: Vec<&str> = image.tags.split("\n").filter(|s| !s.is_empty()).collect();

            let height = if tags.is_empty() { 3 } else { tags.len() + 2 };
            if index < self.info.items.len() - 1 {
                self.info.row_heights.push(height);
            }

            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(row_style)
                .height(height as u16)
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

impl ImageTable {
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if self.err.is_some() {
            self.err = None;
            return Ok(None);
        }

        let event = match key_event.code {
            KeyCode::Delete | KeyCode::Char('d') => self.get_selected_row()
                .map(|i| AppEvent::RemoveImage(i.id.clone(), false)),
            KeyCode::Char('f') => self.get_selected_row()
                .map(|i| AppEvent::RemoveImage(i.id.clone(), true)),
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
        Ok(Some(AppEvent::UpdateImages))
    }

    pub fn show_remove_image_err(&mut self, err: String) {
        let err_msg = Regex::new(REGEX_DELETE_IMG_ERR).ok()
            .and_then(|re| re.find(&err))
            .map(|m| m.as_str())
            .unwrap_or_else(|| "Something went wrong...")
            .to_string();

        self.err = Some(format!("[ERR] {}", err_msg.trim()))
    }
}

impl ImageTableRow {
    const fn ref_array(&self) -> [&String; 4] {
        [&self.id, &self.tags, &self.size, &self.created]
    }

    pub fn from_list(images: Vec<ImageSummary>) -> Vec<Self> {
        let mut result = images.clone();
        result.sort_by_key(|i| i.created);
        result.reverse();
        result.iter().map(Self::from).collect::<Vec<Self>>()
    }

    fn from(image: &ImageSummary) -> Self {
        let id = image.id.split(":").collect::<Vec<&str>>()[1].to_string();

        Self {
            id,
            tags: image.repo_tags.join("\n"),
            size: image.size.to_string(),
            created: time_ago_string(image.created),
        }
    }
}
