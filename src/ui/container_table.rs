use crate::docker::models::{PortMapping, parse_container_state, state_label};
use crate::ui::resource_table::ResourceTableInfo;
use crate::{event::AppEvent, ui::resource_table::ResourceTable, utils::is_container_running};

use super::common::{TableStyle, render_footer};
use bollard::secret::{ContainerStateStatusEnum, ContainerSummary};
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
    state: ContainerStateStatusEnum,
    ports: Vec<PortMapping>,
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

    // The selection index refers to the rendered (filtered) rows, so it must be
    // resolved and wrapped against the visible list, not `items`.
    fn get_selected_row(&mut self) -> Option<&Self::RowType> {
        let index = self.info.state.selected()?;
        let item_index = *self.visible_indices().get(index)?;
        self.info.items.get(item_index)
    }

    fn next_row(&mut self) {
        let Some(last_index) = self.visible_indices().len().checked_sub(1) else {
            return;
        };
        let next_index = self
            .info
            .state
            .selected()
            .map_or(0, |i| if i >= last_index { 0 } else { i + 1 });
        self.select_row(next_index);
    }

    fn previous_row(&mut self) {
        let Some(last_index) = self.visible_indices().len().checked_sub(1) else {
            return;
        };
        let previous_index = self
            .info
            .state
            .selected()
            .map_or(0, |i| if i == 0 { last_index } else { i - 1 });
        self.select_row(previous_index);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = ["ID", "Name", "Image", "State", "Ports"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.info.row_heights.clear();

        let show_all = self.show_all;
        let visible_items: Vec<&ContainerTableRow> = self
            .info
            .items
            .iter()
            .filter(|container| show_all || is_container_running(container.state))
            .collect();
        let visible_count = visible_items.len();

        let rows = visible_items
            .into_iter()
            .enumerate()
            .map(|(index, container)| {
                let row_style = if index % 2 == 0 {
                    self.style.row_style
                } else {
                    self.style.alt_row_style
                };
                let item = container.cells();
                let port_count = container.ports.len();
                let height = if port_count == 0 { 3 } else { port_count + 2 };
                if index < visible_count - 1 {
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

        let is_selected_container_running = self
            .get_selected_row()
            .map(|c| is_container_running(c.state));
        let mut footer_text = get_footer_text(self.show_all, is_selected_container_running);

        if let Some(err) = &self.err {
            border_style = Some(Style::new().red());
            footer_text = err.clone();
        }

        render_footer(frame, area, footer_text, border_style);
    }
}

impl ContainerTable {
    /// Indices into `info.items` for the rows that are currently visible.
    /// Rendering, navigation and selection resolution all share this view.
    fn visible_indices(&self) -> Vec<usize> {
        self.info
            .items
            .iter()
            .enumerate()
            .filter(|(_, container)| self.show_all || is_container_running(container.state))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if self.err.is_some() {
            self.err = None;
            return Ok(None);
        }

        let event = match key_event.code {
            KeyCode::Char('t') => {
                self.show_all = !self.show_all;
                // The old selection index may point past the new visible list; clamp it.
                if let Some(last_index) = self.visible_indices().len().checked_sub(1) {
                    let selected = self.info.state.selected().unwrap_or(0);
                    self.select_row(selected.min(last_index));
                }
                None
            }
            KeyCode::Delete | KeyCode::Char('d') => self
                .get_selected_row()
                .map(|c| AppEvent::RemoveContainer(c.id.clone())),
            KeyCode::Enter => self
                .get_selected_row()
                .map(|c| AppEvent::GoToContainerDetails(c.id.clone())),
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
        if self.info.state.selected().is_none() && !self.visible_indices().is_empty() {
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
        let err_msg = err
            .split(":")
            .collect::<Vec<&str>>()
            .get(2)
            .map_or("Something went wrong...", |v| v);
        self.err = Some(format!("[ERR] {}", err_msg.trim()))
    }
}

impl ContainerTableRow {
    fn cells(&self) -> [String; 5] {
        let ports_text = if self.ports.is_empty() {
            "-".to_string()
        } else {
            self.ports
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("\n")
        };

        [
            self.id.clone(),
            self.name.clone(),
            self.image.clone(),
            state_label(self.state),
            ports_text,
        ]
    }

    pub fn from_list(containers: Vec<ContainerSummary>) -> Vec<Self> {
        let mut result_list = containers
            .iter()
            .map(ContainerTableRow::from)
            .collect::<Vec<ContainerTableRow>>();

        result_list.sort_by(|p, n| {
            let p_is_running = p.state == ContainerStateStatusEnum::RUNNING;
            let n_is_running = n.state == ContainerStateStatusEnum::RUNNING;

            match (p_is_running, n_is_running) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => p.state.as_ref().cmp(n.state.as_ref()),
            }
        });

        result_list
    }

    fn from(container: &ContainerSummary) -> Self {
        let name: String = container
            .names
            .as_deref()
            .and_then(|names| names.first())
            .and_then(|name| name.strip_prefix("/"))
            .map_or("NaN".to_string(), |name| name.to_string());

        Self {
            id: container.id.as_deref().unwrap_or("-").to_string(),
            name,
            image: container.image.as_deref().unwrap_or("-").to_string(),
            state: parse_container_state(container.state.as_deref()),
            ports: container
                .ports
                .as_deref()
                .map(PortMapping::from_summary_ports)
                .unwrap_or_default(),
        }
    }
}

fn get_footer_text(show_all: bool, is_running: Option<bool>) -> String {
    let toggle_text = if show_all { "All" } else { "Running" };
    let mut op_text = "".to_string();

    if let Some(running) = is_running {
        let running_text = if running {
            "restart | <S> stop | <X> kill "
        } else {
            "start "
        };
        op_text = format!(" | <R> {running_text}| <Del/D> remove");
    }

    format!(" <Ent> details | <T> {toggle_text}{op_text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary_with_state(state: &str) -> ContainerSummary {
        ContainerSummary {
            id: Some(format!("id-{state}")),
            names: Some(vec![format!("/{state}")]),
            image: Some("image".to_string()),
            state: Some(state.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn from_list_sorts_running_first_and_others_alphabetically_by_state() {
        let containers = vec![
            summary_with_state("exited"),
            summary_with_state("restarting"),
            summary_with_state("created"),
            summary_with_state("running"),
            summary_with_state("paused"),
        ];

        let rows = ContainerTableRow::from_list(containers);
        let states: Vec<ContainerStateStatusEnum> = rows.iter().map(|r| r.state).collect();

        // Running must come first.
        assert_eq!(states[0], ContainerStateStatusEnum::RUNNING);
        // "restarting" is not "running", so it must not be treated as running.
        assert_ne!(states[1], ContainerStateStatusEnum::RESTARTING);

        // Remaining rows are ordered alphabetically by state name:
        // created < exited < paused < restarting.
        let non_running_states: Vec<ContainerStateStatusEnum> = states[1..].to_vec();
        assert_eq!(
            non_running_states,
            vec![
                ContainerStateStatusEnum::CREATED,
                ContainerStateStatusEnum::EXITED,
                ContainerStateStatusEnum::PAUSED,
                ContainerStateStatusEnum::RESTARTING,
            ]
        );
    }

    #[test]
    fn from_strips_leading_slash_from_name() {
        let container = summary_with_state("running");
        let row = ContainerTableRow::from(&container);
        assert_eq!(row.name, "running");
    }

    #[test]
    fn cells_fall_back_to_dash_for_missing_fields() {
        let container = ContainerSummary::default();
        let row = ContainerTableRow::from(&container);
        let cells = row.cells();

        assert_eq!(cells[0], "-"); // id
        assert_eq!(cells[2], "-"); // image
        assert_eq!(cells[3], "-"); // state (EMPTY)
        assert_eq!(cells[4], "-"); // ports
    }
}
