use std::collections::HashMap;
use crate::{app::Screen, components::component::Component, event::AppEvent, utils::is_container_running};

use super::common::{render_scrollbar, render_footer};
use color_eyre::eyre::Result;
use bollard::secret::{ContainerInspectResponse, ContainerState, ContainerStateStatusEnum, MountPoint, MountPointTypeEnum, PortBinding};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{layout::{Constraint, Layout, Rect}, style::{palette::tailwind, Color, Style, Styled, Stylize}, text::Line, widgets::{Block, BorderType, Paragraph, ScrollbarState}, Frame};

#[derive(Default, Clone)]
pub struct ContainerData {
    id: String,
    name: String,
    image: String,
    created: String,
    state: String,
    ip_address: String,
    start_time: String,
    port_configs: String,
    cmd: String,
    entrypoint: String,
    env: String,
    restart_policy: String,
    volumes: String,
    labels: String
}

impl ContainerData {
    pub fn from(container: ContainerInspectResponse) -> Self {
        let name = container.name.as_deref()
            .and_then(|name| name.strip_prefix("/"))
            .map(String::from)
            .unwrap_or_else(|| "NaN".to_string());

        let config = container.config.as_ref();
        let image = config
            .and_then(|c| c.image.clone())
            .unwrap_or_else(|| "-".to_string());
        let cmd = config
            .and_then(|c| c.cmd.as_ref())
            .map(|cmd| cmd.join("\n"))
            .unwrap_or_else(|| "-".to_string());
        let env = config
            .and_then(|c| c.env.as_ref())
            .map(|env| env.join("\n"))
            .unwrap_or_else(|| "-".to_string());
        let entrypoint = config
            .and_then(|c| c.entrypoint.as_ref())
            .map(|e| e.join("\n"))
            .unwrap_or_else(|| "-".to_string());
        let labels = config
            .and_then(|c| c.labels.as_ref())
            .map(|l| l.iter().map(|(v, d)| format!("{}: {}", v, d)).collect::<Vec<String>>().join("\n"))
            .unwrap_or_else(|| "-".to_string());

        let restart_policy = container.host_config.as_ref()
            .and_then(|c| c.restart_policy.as_ref())
            .and_then(|c| c.name)
            .map(|name| format!("{:?}", name).to_lowercase().replace("_", "-"))
            .unwrap_or_else(|| "-".to_string());

        let network_settings = container.network_settings.as_ref();
        let ip_address = network_settings
            .and_then(|ns| ns.ip_address.clone())
            .unwrap_or_else(|| "-".to_string());

        let port_configs: String = network_settings
            .as_ref()
            .and_then(|ns| ns.ports.as_ref())
            .map(get_ports_text)
            .unwrap_or_else(|| "-".to_string());

        let default_state = ContainerState::default();
        let state_info = container.state.as_ref().unwrap_or(&default_state);
        let state: String = state_info.status.unwrap_or(ContainerStateStatusEnum::EMPTY).to_string();
        let start_time: String = state_info.started_at.as_deref().unwrap_or("-").to_string();

        let mounts = container.mounts.as_ref().map_or("-".to_string(), |mp| get_mounts_text(mp));

        Self {
            id: container.id.as_deref().unwrap_or("-").to_string(),
            name,
            image,
            created: container.created.as_deref().unwrap_or("-").to_string(),
            state,
            ip_address,
            start_time,
            port_configs,
            cmd,
            entrypoint,
            env,
            restart_policy,
            volumes: mounts,
            labels,
        }
    }
}

fn get_ports_text(ports: &HashMap<String, Option<Vec<PortBinding>>>) -> String {
    ports.iter().map(|(port, bindings)| {
        let (ipv4_binding, ipv6_binding) = bindings.as_ref().map(|b| {
            let ipv4 = b.iter().find(|pb| pb.host_ip == Some("0.0.0.0".to_string()));
            let ipv6 = b.iter().find(|pb| pb.host_ip == Some("::".to_string()));
            (ipv4, ipv6)
        }).unwrap_or((None, None));

        let port_number = port.split('/').next().unwrap_or("");
        let protocol = port.split('/').nth(1).unwrap_or("");

        let ipv4_str = ipv4_binding.map(get_port_binding_text).unwrap_or_default();
        let ipv6_str = ipv6_binding.map(get_port_binding_text).unwrap_or_default();

        match (ipv4_str.is_empty(), ipv6_str.is_empty()) {
            (false, false) => format!("{} | {} -> {}/{}", ipv4_str, ipv6_str, port_number, protocol),
            (false, true) => format!("{} -> {}/{}", ipv4_str, port_number, protocol),
            (true, false) => format!("{} -> {}/{}", ipv6_str, port_number, protocol),
            _ => "".to_string(),
        }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn get_port_binding_text(port_binding: &PortBinding) -> String {
    format!("{}:{}", port_binding.host_ip.as_deref().unwrap_or(""), port_binding.host_port.as_deref().unwrap_or(""))
}

fn get_mounts_text(mount_points: &[MountPoint]) -> String {
    let mut mp = mount_points.iter().map(|mp| {
        let source = match mp.typ {
            Some(MountPointTypeEnum::VOLUME { .. }) => mp.name.clone().unwrap_or("-".to_string()),
            Some(_) => mp.source.clone().unwrap_or("-".to_string()),
            None => "-".to_string(),
        };

        let destination = mp.destination.clone().unwrap_or("-".to_string());
        format!("{} -> {}", source, destination)
    }).collect::<Vec<String>>();

    mp.sort_by_key(|v| v.clone());
    mp.join("\n")
}

#[derive(Default)]
pub struct ContainerInfoBlock {
    data: ContainerData,
    vertical_scroll: usize,
    horizontal_scroll: usize,
    vertical_scrollbar_state: ScrollbarState,
    horizontal_scrollbar_state: ScrollbarState,
    line_heights: usize,
    longest_line: usize,
    skipped_tick_count_for_refresh: u8,
}

impl Component for ContainerInfoBlock {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        let mut event = None;
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => event = Some(AppEvent::Back),
            KeyCode::Up | KeyCode::Char('k') => self.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(),
            KeyCode::Right | KeyCode::Char('l') => self.scroll_right(),
            KeyCode::Left | KeyCode::Char('h') => self.scroll_left(),
            KeyCode::Delete | KeyCode::Char('d') => {
                event = Some(AppEvent::RemoveContainer(self.data.id.clone()))
            }
            KeyCode::Char(c) => {
                let container_id = self.data.id.clone();
                event = match c {
                    'r' => Some(AppEvent::RestartContainer(container_id)),
                    's' => Some(AppEvent::StopContainer(container_id)),
                    'x' => Some(AppEvent::KillContainer(container_id)),
                    _ => None
                }
            }
            _ => {}
        }
        Ok(event)
    }

    fn tick(&mut self) -> Result<Option<AppEvent>> {
        let mut event = None;
        if self.skipped_tick_count_for_refresh > 10 {
            self.skipped_tick_count_for_refresh = 0;
            event = Some(AppEvent::UpdateContainerInfo(self.data.id.clone()))
        } else {
            self.skipped_tick_count_for_refresh += 1;
        }
        Ok(event)
    }

    fn is_showing(&self, screen: &Screen) -> bool {
        screen.eq(&Screen::ContainerInfo)
    }
    
    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let vertical_layout = Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Length(3)]);
        let [content_area, horizontal_scrollbar_area, footer_area] = vertical_layout.areas(area);

        let horizontal_layout = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]);
        let [info_area, vertical_scrollbar_area] = horizontal_layout.areas(content_area);

        self.longest_line = 0;
        let content_lines = get_content_as_lines(&self.data);
        content_lines.iter().for_each(|line| {
            let line_len = line.to_string().len();
            if line_len > self.longest_line { self.longest_line = line_len }
        });
        self.longest_line = self.longest_line.saturating_sub(info_area.width as usize - 2);
        self.line_heights = content_lines.len().saturating_sub(info_area.height as usize - 2);

        render_content(
            frame,
            info_area,
            content_lines, 
            self.vertical_scroll, 
            self.horizontal_scroll,
            self.data.name.clone()
        );

        self.vertical_scrollbar_state = self.vertical_scrollbar_state
            .content_length(self.line_heights)
            .position(self.vertical_scroll);
        render_scrollbar(frame, vertical_scrollbar_area, &mut self.vertical_scrollbar_state, true);

        self.horizontal_scrollbar_state = self.horizontal_scrollbar_state
            .content_length(self.longest_line)
            .position(self.horizontal_scroll);
        render_scrollbar(frame, horizontal_scrollbar_area, &mut self.horizontal_scrollbar_state, false);

        render_footer(frame, footer_area, get_footer_text(is_container_running(&self.data.state)));

        Ok(())
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

fn render_content(
    frame: &mut Frame,
    area: Rect,
    lines: Vec<Line<'static>>, 
    vertical_scroll: usize,
    horizontal_scroll: usize,
    container_name: String
) {
    let block_style = Style::new().fg(tailwind::BLUE.c400);

    let title = Line::from(format!("Container: {}", container_name))
        .fg(tailwind::SLATE.c200);

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(block_style)
        .title(title);

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((vertical_scroll as u16, horizontal_scroll as u16))
        .left_aligned();

    frame.render_widget(paragraph, area);
}

fn get_content_as_lines(data: &ContainerData) -> Vec<Line<'static>> {
    let spacer = ("".to_string(), "".to_string());

    let mut lines = vec![
        ("ID: ".to_string(), data.id.clone()),
        ("Image: ".to_string(), data.image.clone()),
        ("Created: ".to_string(), data.created.clone()),
        ("Start Time: ".to_string(), data.start_time.clone()),
        ("Restart Policy: ".to_string(), data.restart_policy.clone()),
        ("State: ".to_string(), data.state.clone()),
        spacer.clone(),
        ("CMD: ".to_string(), data.cmd.clone()),
        ("Entrypoint: ".to_string(), data.entrypoint.clone()),
    ];

    if !data.ip_address.is_empty() {
        lines.extend(vec![spacer.clone(), ("IP Address: ".to_string(), data.ip_address.clone())]);
    }

    if let Some(ports) = get_filtered_list(&data.port_configs) {
        lines.extend(vec![spacer.clone(), ("Port Configs:".to_string(), "".to_string())]);
        lines.extend(ports);
    }

    if let Some(volumes) = get_filtered_list(&data.volumes) {
        lines.extend(vec![spacer.clone(), ("Volumes:".to_string(), "".to_string())]);
        lines.extend(volumes);
    }

    if let Some(env) = get_filtered_list(&data.env) {
        lines.extend(vec![spacer.clone(), ("Env:".to_string(), "".to_string())]);
        lines.extend(env);
    }

    if let Some(labels) = get_filtered_list(&data.labels) {
        lines.extend(vec![spacer.clone(), ("Labels:".to_string(), "".to_string())]);
        lines.extend(labels);
    }

    let key_style = Style::new().fg(Color::Green);

    lines.into_iter()
        .filter(|(_, content)| !content.eq("-"))
        .map(|(key, content)| Line::from_iter([key.set_style(key_style), content.into()]))
        .collect()
}

fn get_filtered_list(data: &str) -> Option<Vec<(String, String)>> {
    let mut splitted_data: Vec<String> = data.split("\n")
        .filter(|p| !p.is_empty())
        .map(|s| s.to_string())
        .collect();

    if splitted_data.is_empty() { return None; }

    splitted_data.sort_unstable();
    Some(splitted_data.iter().map(|d| ("".to_string(), format!(" - {}", d))).collect())
}

fn get_footer_text(is_running: bool) -> String {
    let op_text = if is_running { "| <R> restart | <S> stop | <X> kill " } else { "| <R> start " };
    format!(" <Esc/Q> back {}| <Del/D> remove", op_text)
}

impl ContainerInfoBlock {
    pub fn update_data(&mut self, data: ContainerData) {
        self.data = data;
    }

    fn scroll_down(&mut self) {
        if self.vertical_scroll != self.line_heights { self.vertical_scroll += 1 }
    }

    fn scroll_up(&mut self) {
        if self.vertical_scroll != 0 { self.vertical_scroll -= 1 }
    }

    fn scroll_right(&mut self) {
        if self.horizontal_scroll != self.longest_line { self.horizontal_scroll += 1; }
    }

    fn scroll_left(&mut self) {
        if self.horizontal_scroll != 0 { self.horizontal_scroll -= 1; }
    }
}
