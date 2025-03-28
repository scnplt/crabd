use std::cmp::{max, min};

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Layout, Margin, Rect}, 
    style::{palette::tailwind, Color, Style, Styled, Stylize}, 
    text::{Line, Text}, 
    widgets::{Block, BorderType, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget, Wrap}, 
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
#[derive(Default)]
pub struct ContainerInfo {
    pub data: ContainerInfoData,
    vertical_scroll: usize,
    scrollbar_state: ScrollbarState,
    content_length: usize,
    content_area_size: usize,
}

impl ContainerInfo {
    
    pub fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]);
        let [content_area, footer_area] = vertical.areas(frame.area());
        let buf = frame.buffer_mut();

        let content_lines = get_content_as_lines(&self.data);
        self.content_length = content_lines.len();
        self.content_area_size = content_area.height as usize;

        render_content(content_area, buf, content_lines, self.vertical_scroll, self.data.name.clone());
        self.render_scrollbar(content_area.inner(Margin { vertical: 1, horizontal: 0 }), buf);
        render_footer(footer_area, buf);
    }

    fn render_scrollbar(&mut self, area: Rect, buf: &mut Buffer) {
        self.scrollbar_state = self.scrollbar_state
            .content_length(self.content_length)
            .viewport_content_length(self.content_area_size)
            .position(self.vertical_scroll);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v"));

        scrollbar.render(area, buf, &mut self.scrollbar_state);
    }

    pub fn handle_key_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(),
            _ => {}
        };
    }

    fn scroll_down(&mut self) {
        if self.content_area_size + self.vertical_scroll >= self.content_length { return; }
        self.vertical_scroll = min(self.content_length, self.vertical_scroll + 1);
    }

    fn scroll_up(&mut self) {
        self.vertical_scroll = max(0, self.vertical_scroll.saturating_sub(1));
    }
}

fn render_content(area: Rect, buf: &mut Buffer, lines: Vec<Line<'static>>, vertical_scroll: usize, container_name: String) {
    let block_style = Style::new().fg(tailwind::BLUE.c400);

    let title = Line::from(format!("Container: {}", container_name))
        .fg(tailwind::SLATE.c200);

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(block_style)
        .title(title);

    Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .block(block)
        .scroll((vertical_scroll as u16, 0))
        .left_aligned()
        .render(area, buf);
}

fn get_content_as_lines(data: &ContainerInfoData) -> Vec<Line<'static>> {
    let key_style = Style::new().fg(Color::Green);
    let spacer = ("".to_string(), "".to_string());

    let mut lines = vec![
        ("ID: ".to_string(), data.id.clone()),
        ("Image: ".to_string(), data.image.clone()),
        ("Created: ".to_string(), data.created.clone()),
        ("Start Time: ".to_string(), data.start_time.clone()),
        ("Restart Policies: ".to_string(), data.restart_policies.clone()),
        ("State: ".to_string(), data.state.clone()),
        spacer.clone(),
        ("CMD: ".to_string(), data.cmd.clone()),
        ("Entrypoint: ".to_string(), data.entrypoint.clone()),
        spacer.clone(),
        ("IP Address: ".to_string(), data.ip_address.clone()),
        ("Port Configs:".to_string(), "".to_string()),
    ];

    let mut ports: Vec<&str> = data.port_configs.split("\n").filter(|p| !p.is_empty()).collect();
    ports.sort_unstable();
    lines.extend(ports.iter().map(|port| ("".to_string(), format!(" - {}", port))));

    lines.extend(vec![spacer.clone(), ("Volumes:".to_string(), "".to_string())]);
    let mut volumes: Vec<&str> = data.volumes.split("\n").collect();
    volumes.sort_unstable();
    lines.extend(volumes.iter().map(|volume| ("".to_string(), format!(" - {}", volume))));

    lines.extend(vec![spacer.clone(), ("Env:".to_string(), "".to_string())]);
    let mut env: Vec<&str> = data.env.split("\n").collect();
    env.sort_unstable();
    lines.extend(env.iter().map(|e| ("".to_string(), format!(" - {}", e))));

    lines.into_iter()
        .map(|(key, content)| Line::from_iter([key.set_style(key_style), content.into()]))
        .collect()
}

fn render_footer(area: Rect, buf: &mut Buffer) {
    let footer_style = Style::new().fg(tailwind::SLATE.c200);
    let border_style = Style::new().fg(tailwind::BLUE.c400);

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(border_style);

    let footer = Paragraph::new(Text::from(" <Esc/Q> back | <R> restart | <S> stop | <X> kill | <Del/D> remove"))
        .style(footer_style)
        .left_aligned()
        .block(block);

    footer.render(area, buf);
}

