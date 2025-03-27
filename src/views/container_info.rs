use std::cmp::{max, min};

use crossterm::event::KeyCode;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, FromRepr};
use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Layout, Margin, Rect}, 
    style::{palette::tailwind, Color, Style, Styled, Stylize}, 
    text::{Line, Text}, 
    widgets::{Block, BorderType, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Tabs, Widget}, 
    Frame
};

#[derive(Default, Clone)]
pub struct ContainerInfoData {
    pub id: String,
    pub name: String,
    pub image: String,
    pub created: String,
    pub state: String,
    pub ip_address: String,
    pub start_time: String,
    pub port_configs: String,
    pub cmd: String,
    pub entrypoint: String,
    pub env: String,
    pub restart_policies: String,
    pub volumes: String,
}

#[derive(Default, Clone, Copy, Display, FromRepr, EnumIter)]
enum SelectedTab {
    #[default]
    #[strum(to_string = "Status")]
    Status,
    #[strum(to_string = "Details")]
    Details,
    #[strum(to_string = "Volumes")]
    Volumes,
    #[strum(to_string = "Network")]
    Network,
}

impl SelectedTab {

    fn title(self) -> Line<'static> {
        format!("  {self}  ")
            .fg(tailwind::SLATE.c200)
            .bg(tailwind::BLUE.c900)
            .into()
    }
    
    fn previous(self) -> Self {
        let current_index = self as usize;
        let previouse_index = current_index.saturating_sub(1);
        Self::from_repr(previouse_index).unwrap_or(self)
    }

    fn next(self) -> Self {
        let current_index = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(self)
    }

    fn get_tab_content(self, data: &ContainerInfoData) -> Vec<(String, String)> {
        match self {
            Self::Status => self.get_status_content(data),
            Self::Details => self.get_details_content(data),
            Self::Volumes => self.get_volumes_content(data),
            Self::Network => self.get_network_content(data),
        }
    }

    fn get_status_content(self, data: &ContainerInfoData) -> Vec<(String, String)> {
        vec![
            (" ID: ".to_string(), String::from(&data.id)),
            (" Name: ".to_string(), String::from(&data.name)),
            (" IP Address: ".to_string(), String::from(&data.ip_address)),
            (" State: ".to_string(), String::from(&data.state)),
            (" Created: ".to_string(), String::from(&data.created)),
            (" Start Time: ".to_string(), String::from(&data.start_time)),
        ]
    }

    fn get_details_content(self, data: &ContainerInfoData) -> Vec<(String, String)> {
        let mut lines: Vec<(String, String)> = vec![
            (" Image: ".to_string(), String::from(&data.image)),
            (" CMD: ".to_string(), String::from(&data.cmd)),
            (" Entrypoint: ".to_string(), String::from(&data.entrypoint)),
            (" Restart Policies: ".to_string(), String::from(&data.restart_policies)),
            (" Port Configs: ".to_string(), "".to_string()),
        ];

        let mut ports: Vec<&str> = data.port_configs.split("\n").filter(|p| !p.is_empty()).collect();
        ports.sort_unstable();
        lines.extend(ports.iter().map(|port| ("".to_string(), format!("  - {}", port))));

        lines.push((" ENV: ".to_string(), String::default()));
        let mut envs: Vec<&str> = data.env.split("\n").collect();
        envs.sort_unstable();
        lines.extend(envs.iter().map(|env| ("".to_string(), format!("  - {}", env))));

        lines
    }

    fn get_volumes_content(self, _: &ContainerInfoData) -> Vec<(String, String)> {
        vec![]
    }

    fn get_network_content(self, _: &ContainerInfoData) -> Vec<(String, String)> {
        vec![]
    }
}

#[derive(Default)]
pub struct ContainerInfo {
    pub data: ContainerInfoData,
    selected_tab: SelectedTab,
    vertical_scroll: usize,
    scrollbar_state: ScrollbarState,
    content_length: usize,
    inner_area_size: usize,
}

impl ContainerInfo {
    
    pub fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)]);
        let [header_area, inner_area, footer_area] = vertical.areas(frame.area());
        self.inner_area_size = inner_area.height as usize;

        let horizontal = Layout::horizontal([Constraint::Min(0), Constraint::Length(23)]);
        let [tabs_area, title_area] = horizontal.areas(header_area);

        let buf = frame.buffer_mut();

        render_title(title_area, buf);
        render_tabs(self.selected_tab, tabs_area, buf);
        
        let tab_content: Vec<(String, String)> = self.selected_tab.get_tab_content(&self.data);
        self.content_length = tab_content.len();

        render_tab_content(inner_area, buf, tab_content, self.vertical_scroll);
        self.render_scrollbar(inner_area.inner(Margin { vertical: 1, horizontal: 0 }),buf);

        render_footer(footer_area, buf);
    }

    fn render_scrollbar(&mut self, area: Rect, buf: &mut Buffer) {
        self.scrollbar_state = self.scrollbar_state
            .content_length(self.content_length)
            .viewport_content_length(self.inner_area_size)
            .position(self.vertical_scroll);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v"));

        scrollbar.render(area, buf, &mut self.scrollbar_state);
    }

    pub fn handle_key_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Right | KeyCode::Char('l') => self.selected_tab = self.selected_tab.next(),
            KeyCode::Left | KeyCode::Char('h') => self.selected_tab = self.selected_tab.previous(),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(),
            _ => {}
        };
    }

    fn scroll_down(&mut self) {
        if self.inner_area_size + self.vertical_scroll >= self.content_length { return; }
        self.vertical_scroll = min(self.content_length, self.vertical_scroll + 1);
    }

    fn scroll_up(&mut self) {
        self.vertical_scroll = max(0, self.vertical_scroll.saturating_sub(1));
    }
}

fn render_title(area: Rect, buf: &mut Buffer) {
    "Container Informations".bold().render(area, buf);
}

fn render_tabs(selected_tab: SelectedTab, area: Rect, buf: &mut Buffer) {
    let highlight_style = Style::new()
        .fg(tailwind::SLATE.c950)
        .bg(tailwind::BLUE.c400)
        .bold();

    let titles = SelectedTab::iter().map(SelectedTab::title);
    let selected_tab_index = selected_tab as usize;

    Tabs::new(titles)
        .highlight_style(highlight_style)
        .select(selected_tab_index)
        .padding("", " ")
        .divider("")
        .render(area, buf);
}

fn render_tab_content(area: Rect, buf: &mut Buffer, content: Vec<(String, String)>, vertical_scroll: usize) {
    let key_style = Style::new().fg(Color::Green);
    let block_style = Style::new().fg(tailwind::BLUE.c400);

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(block_style);

    let lines: Vec<Line> = content.into_iter()
        .map(|(key, value)| Line::from_iter([key.set_style(key_style), value.into()]))
        .collect();

    Paragraph::new(lines)
        .block(block)
        .scroll((vertical_scroll as u16, 0))
        .left_aligned()
        .render(area, buf);
}

fn render_footer(area: Rect, buf: &mut Buffer) {
    let footer_style = Style::new().fg(tailwind::SLATE.c200);
    let border_style = Style::new().fg(tailwind::BLUE.c400);

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(border_style);

    let footer = Paragraph::new(Text::from(" <Esc/Q> back"))
        .style(footer_style)
        .left_aligned()
        .block(block);

    footer.render(area, buf);
}

