use std::collections::HashMap;

use bollard::secret::{ContainerInspectResponse, ContainerState, ContainerStateStatusEnum, MountPoint, MountPointTypeEnum, PortBinding};
use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Layout, Rect}, 
    style::{palette::tailwind, Color, Style, Styled, Stylize}, 
    text::Line, 
    widgets::{Block, BorderType, Paragraph, ScrollbarState, Widget, Wrap}, 
    Frame
};

use super::common::{render_footer, render_scrollbar};

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

impl ContainerInfoData {
    
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

        let restart_policies = container.host_config.as_ref()
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
            restart_policies,
            volumes: mounts,
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
pub struct ContainerInfo {
    pub data: ContainerInfoData,
    vertical_scroll: usize,
    scrollbar_state: ScrollbarState,
    content_length: usize,
}

impl ContainerInfo {
    
    pub fn draw(&mut self, frame: &mut Frame) {
        let vertical_layout = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]);
        let [content_area, footer_area] = vertical_layout.areas(frame.area());

        let horizontal_layout = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]);
        let [info_area, scrollbar_area] = horizontal_layout.areas(content_area);

        let content_lines = get_content_as_lines(&self.data);
        self.content_length = content_lines.len().saturating_sub(info_area.height as usize - 2);

        render_content(info_area, frame.buffer_mut(), content_lines, self.vertical_scroll, self.data.name.clone());

        self.scrollbar_state = self.scrollbar_state
            .content_length(self.content_length)
            .position(self.vertical_scroll);

        render_scrollbar(frame, scrollbar_area, &mut self.scrollbar_state, None);

        let is_running = self.data.state == "running";
        render_footer(footer_area, frame.buffer_mut(), get_footer_text(is_running), None, None);
    }

    pub fn handle_key_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => self.scroll_down(),
            _ => {}
        };
    }

    fn scroll_down(&mut self) {
        if self.vertical_scroll == self.content_length { return; }
        self.vertical_scroll += 1;
    }

    fn scroll_up(&mut self) {
        if self.vertical_scroll == 0 { return; }
        self.vertical_scroll -= 1;
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

fn get_footer_text(is_running: bool) -> String {
    let op_text = if is_running { "| <S> stop | <X> kill " } else { "" };
    format!(" <Esc/Q> back | <R> restart {}| <Del/D> remove", op_text)
}
