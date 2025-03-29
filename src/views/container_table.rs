use bollard::secret::{ContainerSummary, Port, PortTypeEnum};
use crossterm::event::KeyCode;
use style::palette::tailwind;
use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{self, Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{Cell, HighlightSpacing, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState},
    Frame,
};

struct TableColors {
    header_fg: Color,
    row_fg: Color,
    selected_row_fg: Color,
    scrollbar_color: Color,
}

impl TableColors {
    const fn new() -> Self {
        Self {
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_fg: tailwind::BLUE.c400,
            scrollbar_color: tailwind::BLUE.c900
        }
    }
}

pub struct ContainerData {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub ports: String
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
    colors: TableColors,
    scrollbar_state: ScrollbarState,
    vertical_scroll: usize,
    row_heights: Vec<usize>,
}

impl ContainersTable {
    pub fn new(items: Vec<ContainerData>) -> Self {
        Self {
            state: TableState::default().with_selected(0),
            items,
            colors: TableColors::new(),
            scrollbar_state: ScrollbarState::default(),
            vertical_scroll: 0,
            row_heights: vec![],
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default().fg(self.colors.header_fg).underlined();

        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_row_fg);

        let header = ["ID", "Name", "Image", "State", "Ports"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        self.row_heights.clear();
        let rows = self.items.iter().map(|data| {
            let item = data.ref_array();
            let ports: Vec<&str> = data.ports.split("\n").filter(|s| !s.is_empty()).collect();    

            let height = if ports.is_empty() { 3 } else { ports.len() + 2 };
            self.row_heights.push(height);

            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(Style::new().fg(self.colors.row_fg))
                .height(height as u16)
        });

        let bar = " ● ";

        let table = Table::new(
            rows,
            vec![
                Constraint::Length(12),
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Percentage(10),
                Constraint::Min(15),
            ],
        )
        .header(header)
        .row_highlight_style(selected_row_style)
        .highlight_symbol(Text::from(vec![
            "".into(),
            bar.into(),
        ]))
        .highlight_spacing(HighlightSpacing::Always);

        let horizontal_layout = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]);
        let [table_area, scrollbar_area] = horizontal_layout.areas(area);

        frame.render_stateful_widget(table, table_area, &mut self.state);
        self.render_scrollbar(frame, scrollbar_area);
    }

    fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        self.row_heights.pop();
        let content_length = self.row_heights.iter().sum::<usize>();

        self.scrollbar_state = self.scrollbar_state
            .content_length(content_length)
            .position(self.vertical_scroll);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v"))
            .style(Style::new().fg(self.colors.scrollbar_color));

        frame.render_stateful_widget(
            scrollbar, 
            area.inner(Margin { vertical: 1, horizontal: 0 }), 
            &mut self.scrollbar_state
        );
    }

    pub fn get_current_container_id(&self) -> String {
        match self.state.selected() {
            Some(index) => self.items.get(index).unwrap().id.clone(),
            _ => "-1".to_string()
        }
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
