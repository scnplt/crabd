use crate::ui::resource_table::ResourceTableInfo;
use crate::{event::AppEvent, ui::resource_table::ResourceTable, utils::is_container_running};

use super::common::{TableStyle, render_footer};
use bollard::secret::{ContainerSummary, Port, PortTypeEnum};
use color_eyre::Result;
use ratatui::style::Stylize;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Rect},
    style::Style,
    text::Text,
    widgets::{Cell, HighlightSpacing, Row, Table},
};

pub struct ContainerTable {
    style: TableStyle,
    show_all: bool,
    skipped_tick_count_for_update: u8,
    info: ResourceTableInfo<ContainerTableRow>,
    err: Option<String>,
}

pub struct ContainerTableRow {
    id: String,
    name: String,
    image: String,
    state: String,
    ports: String,
}

impl Default for ContainerTable {
    fn default() -> Self {
        Self {
            style: TableStyle::default(),
            show_all: true,
            skipped_tick_count_for_update: 0,
            info: ResourceTableInfo::default(),
            err: None,
        }
    }
}

impl ResourceTable for ContainerTable {
    type RowType = ContainerTableRow;

    fn get_table_info(&mut self) -> &mut ResourceTableInfo<Self::RowType> {
        &mut self.info
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = ["ID", "Name", "Image", "State", "Ports"].into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.info.row_heights.clear();

        let rows = self.info.items.iter().enumerate()
            .filter(|(_, container)| self.show_all || String::eq(&container.state, "running"))
            .map(|(index, container)| {
                let row_style = if index % 2 == 0 { self.style.row_style } else { self.style.alt_row_style };
                let item = container.ref_array();
                let ports: Vec<&str> = container.ports.split("\n").filter(|s| !s.is_empty()).collect();

                let height = if ports.is_empty() { 3 } else { ports.len() + 2 };
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
            Constraint::Length(12),
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(10),
            Constraint::Min(15),
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

        let is_selected_container_running = self.get_selected_row().map(|c| is_container_running(&c.state));
        let mut footer_text = get_footer_text(self.show_all, is_selected_container_running);

        if let Some(err) = &self.err {
            border_style = Some(Style::new().red());
            footer_text = err.clone();
        }

        render_footer(frame, area, footer_text, border_style);
    }
}

impl ContainerTable {
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if self.err.is_some() {
            self.err = None;
            return Ok(None);
        }

        let event = match key_event.code {
            KeyCode::Char('t') => {
                self.show_all = !self.show_all;
                None
            }
            KeyCode::Delete | KeyCode::Char('d') => self.get_selected_row().map(|c| AppEvent::RemoveContainer(c.id.clone())),
            KeyCode::Enter => self.get_selected_row().map(|c| AppEvent::GoToContainerDetails(c.id.clone())),
            KeyCode::Char(c) => match (c, self.get_selected_row().map(|c| c.id.clone())) {
                ('r', Some(id)) => Some(AppEvent::RestartContainer(id)),
                ('s', Some(id)) => Some(AppEvent::StopContainer(id)),
                ('x', Some(id)) => Some(AppEvent::KillContainer(id)),
                _ => self.handle_nav_key_event(key_event)?,
            },
            _ => None,
        };

        Ok(event)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let result = self.draw_default(frame, area);

        // If there is items but no row selected, select the first row.
        // This happens when changing the `self.show_all` parameter.
        if self.info.state.selected().is_none() && !self.info.items.is_empty() {
            self.select_row(0);
        }

        result
    }

    pub fn tick(&mut self) -> Result<Option<AppEvent>> {
        if self.skipped_tick_count_for_update <= 10 {
            self.skipped_tick_count_for_update += 1;
            return Ok(None);
        }

        self.skipped_tick_count_for_update = 0;
        Ok(Some(AppEvent::UpdateContainers))
    }

    pub fn show_container_err(&mut self, err: String) {
        let err_msg = err.split(":") .collect::<Vec<&str>>().get(2)
            .map_or("Something went wrong...", |v| v);
        self.err = Some(format!("[ERR] {}", err_msg.trim()))
    }
}

impl ContainerTableRow {
    const fn ref_array(&self) -> [&String; 5] {
        [&self.id, &self.name, &self.image, &self.state, &self.ports]
    }

    pub fn from_list(containers: Vec<ContainerSummary>) -> Vec<Self> {
        let mut result_list = containers.iter()
            .map(ContainerTableRow::from)
            .collect::<Vec<ContainerTableRow>>();

        result_list.sort_by(|p, n| {
            let p_is_running = p.state.starts_with("r");
            let n_is_running = n.state.starts_with("r");

            match (p_is_running, n_is_running) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => p.state.cmp(&n.state),
            }
        });

        result_list
    }

    fn from(container: &ContainerSummary) -> Self {
        let name: String = container.names.as_deref()
            .and_then(|names| names.first())
            .and_then(|name| name.strip_prefix("/"))
            .map_or("NaN".to_string(), |name| name.to_string());

        Self {
            id: container.id.as_deref().unwrap_or("-").to_string(),
            name,
            image: container.image.as_deref().unwrap_or("-").to_string(),
            state: container.state.as_deref().unwrap_or("-").to_string(),
            ports: container.ports.as_ref().map_or("-".to_string(), |p| get_ports_text(p)),
        }
    }
}

fn get_ports_text(ports: &[Port]) -> String {
    let mut filtered_ports: Vec<(u16, u16, PortTypeEnum)> = ports.iter()
        .filter_map(|p| Some((p.private_port, p.public_port?, p.typ?)))
        .collect();

    filtered_ports.sort_by_key(|&(private, _, _)| private);
    filtered_ports.dedup();

    filtered_ports.iter()
        .map(|&(private, public, typ)| format!("{private}:{public}/{typ}"))
        .collect::<Vec<String>>()
        .join("\n")
}

fn get_footer_text(show_all: bool, is_running: Option<bool>) -> String {
    let toggle_text = if show_all { "All" } else { "Running" };
    let mut op_text = "".to_string();

    if let Some(running) = is_running {
        let running_text = if running { "restart | <S> stop | <X> kill " } else { "start " };
        op_text = format!(" | <R> {running_text}| <Del/D> remove");
    }

    format!(" <Ent> details | <T> {toggle_text}{op_text}")
}
