use bollard::secret::{ContainerSummary, Port, PortTypeEnum};
use crossterm::event::KeyCode;
use style::palette::tailwind;
use ratatui::{
    layout::{Constraint, Rect},
    style::{self, Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Cell, HighlightSpacing, 
        Row, Table, TableState,
    },
    Frame,
};

struct TableColors {
    header_fg: Color,
    row_fg: Color,
    selected_row_fg: Color,
}

impl TableColors {
    const fn new() -> Self {
        Self {
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_fg: tailwind::BLUE.c400,
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
    colors: TableColors
}

impl ContainersTable {
    pub fn new(items: Vec<ContainerData>) -> Self {
        Self {
            state: TableState::default().with_selected(0),
            items,
            colors: TableColors::new()
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

        let rows = self.items.iter().map(|data| {
            let item = data.ref_array();
            let ports: Vec<&str> = data.ports.split("\n").collect();            

            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(Style::new().fg(self.colors.row_fg))
                .height((ports.len() + 2) as u16)
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

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    pub fn get_current_container_id(&self) -> String {
        if let Some(index) = self.state.selected() {
            let container = self.items.get(index).unwrap();
            return container.id.clone();
        }

        "-1".to_string()
    }

    pub fn handle_key_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.next_row(),
            KeyCode::Char('k') | KeyCode::Up => self.previous_row(),
            _ => {}
        }
    }

    fn next_row(&mut self) {
        let index = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(index));
    }

    fn previous_row(&mut self) {
        let index = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(index));
    }
}
