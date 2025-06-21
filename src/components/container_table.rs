use crate::{event::AppEvent, utils::is_container_running};

use super::common::{TableStyle, render_footer, render_scrollbar};
use bollard::secret::{ContainerSummary, Port, PortTypeEnum};
use color_eyre::Result;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    text::Text,
    widgets::{Cell, HighlightSpacing, Row, ScrollbarState, Table, TableState},
};

pub struct ContainerTable {
    state: TableState,
    items: Vec<ContainerTableRow>,
    vertical_scrollbar_state: ScrollbarState,
    style: TableStyle,
    row_heights: Vec<usize>,
    show_all: bool,
    vertical_scroll: usize,
    skipped_tick_count_for_update: u8,
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
            state: TableState::default().with_selected(0),
            items: vec![],
            vertical_scrollbar_state: ScrollbarState::default(),
            style: TableStyle::default(),
            row_heights: vec![],
            show_all: true,
            vertical_scroll: 0,
            skipped_tick_count_for_update: 0,
        }
    }
}

impl ContainerTable {
    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        let mut event = None;
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => event = Some(AppEvent::Quit),
            KeyCode::Down | KeyCode::Char('j') => self.next_row(),
            KeyCode::Up | KeyCode::Char('k') => self.previous_row(),
            KeyCode::Char('t') => self.show_all = !self.show_all,
            KeyCode::Delete | KeyCode::Char('d') => {
                if let Some(container) = self.get_selected_container() {
                    event = Some(AppEvent::RemoveContainer(container.id.clone()))
                }
            }
            KeyCode::Enter => {
                if let Some(container) = self.get_selected_container() {
                    event = Some(AppEvent::GoToContainerDetails(container.id.clone()))
                }
            }
            KeyCode::Char(c) => {
                event = match (c, self.get_selected_container().map(|c| c.id.clone())) {
                    ('r', Some(id)) => Some(AppEvent::RestartContainer(id)),
                    ('s', Some(id)) => Some(AppEvent::StopContainer(id)),
                    ('x', Some(id)) => Some(AppEvent::KillContainer(id)),
                    _ => None,
                };
            }
            _ => {}
        };
        Ok(event)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        use Constraint::{Length, Min};

        let vertical_layout = Layout::vertical([Min(0), Length(3)]);
        let [content_area, footer_area] = vertical_layout.areas(area);

        let horizontal_content_layout = Layout::horizontal([Min(0), Length(1)]);
        let [table_area, scrollbar_area] = horizontal_content_layout.areas(content_area);

        self.render_table(frame, table_area);

        self.update_scroll_state();
        render_scrollbar(
            frame,
            scrollbar_area,
            &mut self.vertical_scrollbar_state,
            true,
        );

        let is_selected_container_running = self.get_selected_container()
            .map(|c| is_container_running(&c.state));

        let footer_text = get_footer_text(self.show_all, is_selected_container_running);
        render_footer(frame, footer_area, footer_text, None);

        // If there is items but no row selected, select the first row.
        // This happens when changing the `self.show_all` parameter.
        if self.state.selected().is_none() && !self.items.is_empty() {
            self.select_row(0);
        }

        Ok(())
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = ["ID", "Name", "Image", "State", "Ports"].into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.style.header_style)
            .height(1);

        self.row_heights.clear();

        let rows = self.items.iter().enumerate()
            .filter(|(_, container)| self.show_all || String::eq(&container.state, "running"))
            .map(|(index, container)| {
                let row_style = if index % 2 == 0 { self.style.row_style } else { self.style.alt_row_style };
                let item = container.ref_array();
                let ports: Vec<&str> = container.ports.split("\n").filter(|s| !s.is_empty()).collect();

                let height = if ports.is_empty() { 3 } else { ports.len() + 2 };
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

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    fn update_scroll_state(&mut self) {
        let content_height = self.row_heights.iter().sum::<usize>();
        self.vertical_scrollbar_state = self.vertical_scrollbar_state
            .content_length(content_height)
            .position(self.vertical_scroll);
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

    pub fn update_with_items(&mut self, items: Vec<ContainerTableRow>) {
        let is_empty_before_update = self.items.is_empty();
        self.items = items;
        if is_empty_before_update && !self.items.is_empty() {
            self.select_row(0);
        }
    }

    pub fn get_selected_container(&self) -> Option<&ContainerTableRow> {
        self.state.selected().and_then(|index| self.items.get(index))
    }

    pub fn tick(&mut self) -> Result<Option<AppEvent>> {
        if self.skipped_tick_count_for_update <= 10 {
            self.skipped_tick_count_for_update += 1;
            return Ok(None);
        }

        self.skipped_tick_count_for_update = 0;
        Ok(Some(AppEvent::UpdateContainers))
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
        let running_text = if running {
            "restart | <S> stop | <X> kill "
        } else {
            "start "
        };
        op_text = format!(" | <R> {running_text}| <Del/D> remove");
    }

    format!(" <Ent> details | <T> {toggle_text}{op_text}")
}
