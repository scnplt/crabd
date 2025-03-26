use crossterm::event::KeyCode;
use style::palette::tailwind;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{self, Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, BorderType, Cell, HighlightSpacing, 
        Paragraph, Row, Table, TableState,
    },
    Frame,
};

struct TableColors {
    header_fg: Color,
    row_fg: Color,
    selected_row_fg: Color,
    footer_border_color: Color,
}

impl TableColors {
    const fn new() -> Self {
        Self {
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_fg: tailwind::BLUE.c400,
            footer_border_color: tailwind::BLUE.c400,
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

    pub fn draw(&mut self, frame: &mut Frame) {
        let vertical = &Layout::vertical([Constraint::Min(5), Constraint::Length(3)]);
        let rects = vertical.split(frame.area());

        self.render_table(frame, rects[0]);
        self.render_footer(frame, rects[1]);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
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

    fn render_footer(&mut self, frame: &mut Frame, area: Rect) {
        let footer_style = Style::new().fg(self.colors.row_fg);
        let block_style = Style::new().fg(self.colors.footer_border_color);

        let block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(block_style);

        let footer = Paragraph::new(Text::from("<Ent> details | <T> view mode | <R> restart | <S> stop | <X> kill | <D/Del> remove"))
            .style(footer_style)
            .left_aligned()
            .block(block);
        
        frame.render_widget(footer, area);
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
