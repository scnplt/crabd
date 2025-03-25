use color_eyre::{owo_colors::OwoColorize, Result};
use crossterm::event::KeyModifiers;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Margin, Rect},
    style::{self, Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
    DefaultTerminal, Frame,
};
use style::palette::tailwind;

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_row_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
}

impl TableColors {
    const fn new() -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: tailwind::BLUE.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_fg: tailwind::BLUE.c400,
            normal_row_color: tailwind::SLATE.c950,
            alt_row_color: tailwind::SLATE.c900,
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
    items: Vec<ContainerData>,
    selected_index: usize,
    colors: TableColors
}

impl ContainersTable {
    pub fn new(items: Vec<ContainerData>) -> Self {
        Self {
            state: TableState::default().with_selected(0),
            items,
            selected_index: 0,
            colors: TableColors::new()
        }
    }

    pub fn next_row(&mut self) {
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

    pub fn previous_row(&mut self) {
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

    pub fn render_table(&mut self, frame: &mut Frame) {
        let vertical = &Layout::vertical([Constraint::Min(5), Constraint::Length(4)]);
        let rects = vertical.split(frame.area());

        let header_style = Style::default()
            .fg(self.colors.header_fg)
            .bg(self.colors.header_bg);
        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_row_fg);

        let header = ["ID", "Name", "Image", "State", "Pors"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = self.items.iter().enumerate().map(|(index, data)| {
            let color = match index % 2 {
                0 => self.colors.normal_row_color,
                _ => self.colors.alt_row_color,
            };
            let item = data.ref_array();
            item.into_iter()
                .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                .collect::<Row>()
                .style(Style::new().fg(self.colors.row_fg).bg(color))
                .height(4)
        });

        let bar = " █ ";

        let table = Table::new(
            rows,
            vec![
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
            ],
        )
        .header(header)
        .row_highlight_style(selected_row_style)
        .bg(self.colors.buffer_bg)
        .highlight_symbol(Text::from(vec![
            "".into(),
            bar.into(),
            bar.into(),
            "".into()
        ]))
        .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(table, rects[0], &mut self.state);
    }
}
