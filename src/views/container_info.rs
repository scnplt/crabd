use crossterm::event::KeyCode;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, FromRepr};
use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Layout, Rect}, 
    style::{palette::tailwind, Color, Style, Styled, Stylize}, 
    text::{Line, Text}, 
    widgets::{Block, BorderType, Padding, Paragraph, Tabs, Widget}, 
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

    fn render(self, area: Rect, buf: &mut Buffer, data: &ContainerInfoData) {
        match self {
            Self::Status => self.render_status_tab(area, buf, data),
            Self::Details => self.render_details_tab(area, buf, data),
            Self::Volumes => self.render_volumes_tab(area, buf, data),
            Self::Network => self.render_network_tab(area, buf, data),
        }
    }

    fn render_status_tab(self, area: Rect, buf: &mut Buffer, data: &ContainerInfoData) {
        let key_style = Style::new().fg(Color::Green);

        let lines = [
            (" ID: ", data.id.clone()),
            (" Name: ", data.name.clone()),
            (" IP Address: ", data.ip_address.clone()),
            (" State: ", data.state.clone()),
            (" Created: ", data.created.clone()),
            (" Start Time: ", data.start_time.clone()),
        ].into_iter()
        .map(|(key, content)| Line::from_iter([key.set_style(key_style), content.into()]))
        .collect::<Vec<Line>>();

        let block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(Style::new().fg(tailwind::BLUE.c400));

        Paragraph::new(lines)
            .block(block)
            .left_aligned()
            .render(area, buf);
    }

    fn render_details_tab(self, area: Rect, buf: &mut Buffer, data: &ContainerInfoData) {
        let key_style = Style::new().fg(Color::Green);

        let mut lines = [
            (" Image: ", data.image.clone()),
            (" CMD: ", data.cmd.clone()),
            (" Entrypoint: ", data.entrypoint.clone()),
            (" Restart Policies: ", data.restart_policies.clone()),
        ].into_iter()
        .map(|(key, content)| Line::from_iter([key.set_style(key_style), content.into()]))
        .collect::<Vec<Line>>();

        lines.push(Line::from(" Port Configs: ".set_style(key_style)));
        let mut ports: Vec<&str> = data.port_configs.split("\n").collect();
        ports.sort_by_key(|p| *p);
        ports.iter()
            .filter(|p| !p.is_empty())
            .for_each(|p| lines.push(Line::from(format!("  - {}", p))));

        
        lines.push(Line::from(" ENV: ".set_style(key_style)));
        let mut envs: Vec<&str> = data.env.split("\n").collect();
        envs.sort_by_key(|env| *env);
        envs.iter().for_each(|env| lines.push(Line::from(format!("  - {}", env))));

        let block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(Style::new().fg(tailwind::BLUE.c400));

        Paragraph::new(lines)
            .block(block)
            .left_aligned()
            .render(area, buf);
    }

    fn render_volumes_tab(self, area: Rect, buf: &mut Buffer, data: &ContainerInfoData) {
        // TODO
        Block::new().render(area, buf);
    }

    fn render_network_tab(self, area: Rect, buf: &mut Buffer, data: &ContainerInfoData) {
        // TODO
        Block::new().render(area, buf);
    }
}

#[derive(Default)]
pub struct ContainerInfo {
    pub data: ContainerInfoData,
    selected_tab: SelectedTab
}

impl ContainerInfo {
    
    pub fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(3)]);
        let [header_area, inner_area, footer_area] = vertical.areas(frame.area());

        let horizontal = Layout::horizontal([Constraint::Min(0), Constraint::Length(23)]);
        let [tabs_area, title_area] = horizontal.areas(header_area);

        let buf = frame.buffer_mut();

        render_title(title_area, buf);
        render_tabs(self.selected_tab, tabs_area, buf);
        self.selected_tab.render(inner_area, buf, &self.data);
        render_footer(footer_area, buf);
    }

    pub fn handle_key_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Right | KeyCode::Char('l') => self.selected_tab = self.selected_tab.next(),
            KeyCode::Left | KeyCode::Char('h') => self.selected_tab = self.selected_tab.previous(),
            _ => {}
        };
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

fn render_footer(area: Rect, buf: &mut Buffer) {
    let footer_style = Style::new().fg(tailwind::SLATE.c200);
    let border_style = Style::new().fg(tailwind::BLUE.c400);

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(border_style);

    let footer = Paragraph::new(Text::from(" <Q/Esc> back"))
        .style(footer_style)
        .left_aligned()
        .block(block);

    footer.render(area, buf);
}

