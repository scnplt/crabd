use bollard::secret::{ContainerSummary, Port, PortTypeEnum};
use crossterm::event::KeyCode;
use style::palette::tailwind;
use ratatui::{
    layout::{Constraint, Layout, Rect}, 
    style::{self, Modifier, Style}, 
    text::Text, 
    widgets::{Cell, HighlightSpacing, Row, ScrollbarState, Table, TableState}, 
    Frame
};

use super::common::{render_footer, render_scrollbar};

struct TableStyles {
    header_style: Style,
    selected_row_style: Style,
    row_style: Style,
    alt_row_style: Style,
}

impl TableStyles {
    fn default() -> Self {
        Self {
            header_style: Style::default().fg(tailwind::SLATE.c200),
            selected_row_style: Style::default().add_modifier(Modifier::REVERSED).fg(tailwind::BLUE.c400),
            row_style: Style::default().bg(tailwind::SLATE.c800).fg(tailwind::SLATE.c200),
            alt_row_style: Style::default().bg(tailwind::SLATE.c950),
        }
    }
}

pub struct ContainerData {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub ports: String,
}

impl ContainerData {
    const fn ref_array(&self) -> [&String; 5] {
        [&self.id, &self.name, &self.image, &self.state, &self.ports]
    }

    pub fn from_list(containers: Vec<ContainerSummary>, show_all: bool) -> Vec<Self> {
        let mut result_list = containers.iter()
            .filter(|container| {
                let state = container.state.as_deref().unwrap_or("-").to_string();
                if show_all { true } else { String::eq(&state, "running") }
            })
            .map(ContainerData::from)
            .collect::<Vec<ContainerData>>();
    
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

    pub fn from(container: &ContainerSummary) -> Self {
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
        .filter(|p| p.public_port.is_some())
        .map(|p| (p.private_port, p.public_port.unwrap(), p.typ.unwrap()))
        .collect();

    filtered_ports.sort_by_key(|&(private, _, _)| private);
    filtered_ports.dedup();
    
    filtered_ports.iter()
        .map(|&(private, public, typ)| format!("{}:{}/{}", private, public, typ))
        .collect::<Vec<String>>()
        .join("\n")
}

pub struct ContainersTable {
    state: TableState,
    pub items: Vec<ContainerData>,
    scrollbar_state: ScrollbarState,
    vertical_scroll: usize,
    row_heights: Vec<usize>,
    styles: TableStyles,
}

impl ContainersTable {
    pub fn new(items: Vec<ContainerData>) -> Self {
        Self {
            state: TableState::default().with_selected(0),
            items,
            scrollbar_state: ScrollbarState::default(),
            vertical_scroll: 0,
            row_heights: vec![],
            styles: TableStyles::default(),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, show_all: bool) {
        let vertical_layout = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]);
        let [content_area, footer_area] = vertical_layout.areas(frame.area());

        let horizontal_layout = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]);
        let [table_area, scrollbar_area] = horizontal_layout.areas(content_area);

        
        self.render_table(frame, table_area);
        
        let content_height = self.row_heights.iter().sum::<usize>();
        self.scrollbar_state = self.scrollbar_state
            .content_length(content_height)
            .position(self.vertical_scroll);

        render_scrollbar(frame, scrollbar_area, &mut self.scrollbar_state, None);

        let footer_text = get_footer_text(show_all, self.is_selected_container_running());
        render_footer(footer_area, frame.buffer_mut(), footer_text, None, None);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        self.row_heights.clear();

        let header = ["ID", "Name", "Image", "State", "Ports"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(self.styles.header_style)
            .height(1);
    
        self.row_heights.clear();
        let rows = self.items.iter().enumerate().map(|(index, data)| {
            let style = if index % 2 == 0 { self.styles.row_style } else { self.styles.alt_row_style };
            let item = data.ref_array();
            let ports: Vec<&str> = data.ports.split("\n").filter(|s| !s.is_empty()).collect();    
    
            let height = if ports.is_empty() { 3 } else { ports.len() + 2 };
            if index < self.items.len() - 1 { self.row_heights.push(height); }
    
            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(style)
                .height(height as u16)
        });
    
        let bar = " ● ";
        let widths = vec![
            Constraint::Length(12),
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(10),
            Constraint::Min(15),
        ];
    
        let table = Table::new(rows,widths)
            .header(header)
            .row_highlight_style(self.styles.selected_row_style)
            .highlight_symbol(Text::from(vec![
                "".into(),
                bar.into(),
            ]))
            .highlight_spacing(HighlightSpacing::Always);
    
        frame.render_stateful_widget(table, area, &mut self.state);
    }

    pub fn get_current_container_id(&self) -> String {
        match self.state.selected() {
            Some(index) => self.items.get(index).unwrap().id.clone(),
            _ => "-1".to_string()
        }
    }

    fn is_selected_container_running(&self) -> Option<bool> {
        self.state.selected().map(|index| self.items.get(index).unwrap().state == "running")
    }

    pub fn handle_key_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.next_row(),
            KeyCode::Char('k') | KeyCode::Up => self.previous_row(),
            _ => {}
        }
    }

    fn next_row(&mut self) {
        let index = self.state.selected().map_or(0, |i| if i >= self.items.len() - 1 { 0 } else { i + 1 });
        self.state.select(Some(index));
        self.update_vertical_scroll(index);
    }

    fn previous_row(&mut self) {
        let index = self.state.selected().map_or(0, |i| if i == 0 { self.items.len() - 1 } else { i - 1 });
        self.state.select(Some(index));
        self.update_vertical_scroll(index);
    }

    fn update_vertical_scroll(&mut self, index: usize) {
        self.vertical_scroll = self.row_heights.iter().take(index).sum();
    }
}

fn get_footer_text(show_all: bool, is_running: Option<bool>) -> String {
    let toggle_text = if show_all { "All" } else { "Running" };
    let mut op_text = "".to_string();

    if let Some(running) = is_running {
        let running_text = if running { "restart | <S> stop | <X> kill " } else { "start " };
        op_text = format!(" | <R> {}| <Del/D> remove", running_text);
    }

    format!(" <Ent> details | <T> {}{}", toggle_text, op_text)
}
