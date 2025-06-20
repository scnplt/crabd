use std::collections::HashMap;

use crate::{event::AppEvent, utils::is_container_running};

use super::common::{render_footer, render_scrollbar};
use bollard::secret::{
    ContainerInspectResponse, ContainerStateStatusEnum, MountPoint, MountPointTypeEnum, PortBinding,
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Styled, Stylize, palette::tailwind},
    text::Line,
    widgets::{Block, BorderType, Paragraph},
};

use super::info_block::{ScrollInfo, ScrollableInfoBlock};

#[derive(Default, Clone)]
pub struct ContainerInfoBlock {
    data: ContainerData,
    scroll_info: ScrollInfo,
    skipped_tick_count_for_refresh: u8,
}

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
    labels: String,
}

impl ScrollableInfoBlock for ContainerInfoBlock {
    type Data = ContainerData;

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        let event = match key_event.code {
            KeyCode::Delete | KeyCode::Char('d') => {
                Some(AppEvent::RemoveContainer(self.data.id.clone()))
            }
            KeyCode::Char('r') => Some(AppEvent::RestartContainer(self.data.id.clone())),
            KeyCode::Char('s') => Some(AppEvent::StopContainer(self.data.id.clone())),
            KeyCode::Char('x') => Some(AppEvent::KillContainer(self.data.id.clone())),
            _ => self.handle_nav_key_event(key_event)?,
        };
        Ok(event)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        use Constraint::{Length, Min};

        let vertical_layout = Layout::vertical([Min(0), Length(1), Length(3)]);
        let [content_area, horizontal_scrollbar_area, footer_area] = vertical_layout.areas(area);

        let horizontal_layout = Layout::horizontal([Min(0), Length(1)]);
        let [info_area, vertical_scrollbar_area] = horizontal_layout.areas(content_area);

        let content_lines = get_content_as_lines(&self.data);

        let max_horizontal = content_lines.iter().fold(0, |max, line| {
            let line_len = line.to_string().len();
            if line_len > max { line_len } else { max }
        });

        self.scroll_info.max_horizontal = max_horizontal.saturating_sub(info_area.width as usize - 2);
        self.scroll_info.max_vertical = content_lines.len().saturating_sub(info_area.height as usize - 2);

        self.render_content(frame, info_area, content_lines);

        self.scroll_info.vertical_state = self.scroll_info.vertical_state
            .content_length(self.scroll_info.max_horizontal)
            .position(self.scroll_info.vertical);

        render_scrollbar(
            frame,
            vertical_scrollbar_area,
            &mut self.scroll_info.vertical_state,
            true,
        );

        self.scroll_info.horizontal_state = self.scroll_info.horizontal_state
            .content_length(self.scroll_info.max_horizontal)
            .position(self.scroll_info.horizontal);

        render_scrollbar(
            frame,
            horizontal_scrollbar_area,
            &mut self.scroll_info.horizontal_state,
            false,
        );

        render_footer(
            frame,
            footer_area,
            get_footer_text(is_container_running(&self.data.state)),
        );

        Ok(())
    }

    fn tick(&mut self) -> Result<Option<AppEvent>> {
        let event = if self.skipped_tick_count_for_refresh > 10 {
            self.skipped_tick_count_for_refresh = 0;
            Some(AppEvent::UpdateContainerInfo(self.data.id.clone()))
        } else {
            self.skipped_tick_count_for_refresh += 1;
            None
        };
        Ok(event)
    }

    fn update_data(&mut self, data: Self::Data) {
        self.data = data;
    }

    fn get_scroll_info(&mut self) -> &mut super::info_block::ScrollInfo {
        &mut self.scroll_info
    }
}

impl ContainerInfoBlock {
    fn render_content(&mut self, frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
        let block_style = Style::new().fg(tailwind::BLUE.c400);

        let title =
            Line::from(format!("Container: {}", self.data.name.clone())).fg(tailwind::SLATE.c200);

        let block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(block_style)
            .title(title);

        let paragraph = Paragraph::new(lines)
            .block(block)
            .scroll((
                self.scroll_info.vertical as u16,
                self.scroll_info.horizontal as u16,
            ))
            .left_aligned();

        frame.render_widget(paragraph, area);
    }
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
        lines.extend(vec![
            spacer.clone(),
            ("IP Address: ".to_string(), data.ip_address.clone()),
        ]);
    }

    if let Some(ports) = get_filtered_list(&data.port_configs) {
        lines.extend(vec![
            spacer.clone(),
            ("Port Configs:".to_string(), "".to_string()),
        ]);
        lines.extend(ports);
    }

    if let Some(volumes) = get_filtered_list(&data.volumes) {
        lines.extend(vec![
            spacer.clone(),
            ("Volumes:".to_string(), "".to_string()),
        ]);
        lines.extend(volumes);
    }

    if let Some(env) = get_filtered_list(&data.env) {
        lines.extend(vec![spacer.clone(), ("Env:".to_string(), "".to_string())]);
        lines.extend(env);
    }

    if let Some(labels) = get_filtered_list(&data.labels) {
        lines.extend(vec![
            spacer.clone(),
            ("Labels:".to_string(), "".to_string()),
        ]);
        lines.extend(labels);
    }

    let key_style = Style::new().fg(Color::Green);

    lines
        .into_iter()
        .filter(|(_, content)| !content.eq("-"))
        .map(|(key, content)| Line::from_iter([key.set_style(key_style), content.into()]))
        .collect()
}

fn get_filtered_list(data: &str) -> Option<Vec<(String, String)>> {
    let mut splitted_data: Vec<String> = data.split("\n")
        .filter(|p| !p.is_empty())
        .map(|s| s.to_string())
        .collect();

    if splitted_data.is_empty() {
        return None;
    }

    splitted_data.sort_unstable();
    Some(splitted_data.iter().map(|d| ("".to_string(), format!(" - {d}"))).collect())
}

fn get_footer_text(is_running: bool) -> String {
    let op_text = if is_running {
        "| <R> restart | <S> stop | <X> kill "
    } else {
        "| <R> start "
    };
    format!(" <Esc/Q> back {op_text}| <Del/D> remove")
}

impl ContainerData {
    pub fn from(container: ContainerInspectResponse) -> Self {
        let name = container.name.as_deref()
            .and_then(|name| name.strip_prefix("/"))
            .map(String::from)
            .unwrap_or_else(|| "NaN".to_string());

        let restart_policy = container.host_config.as_ref()
            .and_then(|c| c.restart_policy.as_ref())
            .and_then(|c| c.name)
            .map(|name| format!("{name:?}").to_lowercase().replace("_", "-"))
            .unwrap_or_else(|| "-".to_string());

        let mut image = "-".to_string();
        let mut cmd = "-".to_string();
        let mut env = "-".to_string();
        let mut entrypoint = "-".to_string();
        let mut labels = "-".to_string();
        if let Some(config) = container.config {
            image = config.image.unwrap_or(image);
            cmd = config.cmd.map(|c| c.join("\n")).unwrap_or(cmd);
            env = config.env.map(|e| e.join("\n")).unwrap_or(env);
            entrypoint = config.entrypoint.map(|ep| ep.join("\n")).unwrap_or(entrypoint);
            labels = config.labels.map(|l| {
                l.iter()
                    .map(|(v, d)| format!("{v}: {d}"))
                    .collect::<Vec<String>>()
                    .join("\n")
            }).unwrap_or(labels)
        }

        let mut ip_address = "-".to_string();
        let mut port_configs = "-".to_string();
        if let Some(network_settings) = container.network_settings {
            ip_address = network_settings.ip_address.unwrap_or(ip_address);
            port_configs = network_settings.ports
                .map(|p| get_ports_text(&p))
                .unwrap_or(port_configs);
        }

        let mut state = ContainerStateStatusEnum::EMPTY.to_string();
        let mut start_time = "-".to_string();
        if let Some(state_info) = container.state {
            state = state_info.status.map(|s| s.to_string()).unwrap_or(state);
            start_time = state_info.started_at.unwrap_or(start_time);
        }

        let volumes = container.mounts.as_ref().map_or("-".to_string(), |mp| get_mounts_text(mp));

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
            volumes,
            labels,
        }
    }
}

fn get_ports_text(ports: &HashMap<String, Option<Vec<PortBinding>>>) -> String {
    ports
        .iter()
        .map(|(port, bindings)| {
            let (ipv4_binding, ipv6_binding) = bindings
                .as_ref()
                .map(|b| {
                    let ipv4 = b.iter().find(|pb| pb.host_ip == Some("0.0.0.0".to_string()));
                    let ipv6 = b.iter().find(|pb| pb.host_ip == Some("::".to_string()));
                    (ipv4, ipv6)
                })
                .unwrap_or((None, None));

            let port_number = port.split('/').next().unwrap_or("");
            let protocol = port.split('/').nth(1).unwrap_or("");

            let ipv4_str = ipv4_binding.map(get_port_binding_text).unwrap_or_default();
            let ipv6_str = ipv6_binding.map(get_port_binding_text).unwrap_or_default();

            match (ipv4_str.is_empty(), ipv6_str.is_empty()) {
                (false, false) => format!("{ipv4_str} | {ipv6_str} -> {port_number}/{protocol}"),
                (false, true) => format!("{ipv4_str} -> {port_number}/{protocol}"),
                (true, false) => format!("{ipv6_str} -> {port_number}/{protocol}"),
                _ => "".to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn get_port_binding_text(port_binding: &PortBinding) -> String {
    format!(
        "{}:{}",
        port_binding.host_ip.as_deref().unwrap_or(""),
        port_binding.host_port.as_deref().unwrap_or("")
    )
}

fn get_mounts_text(mount_points: &[MountPoint]) -> String {
    let mut mp = mount_points.iter()
        .map(|mp| {
            let source = match mp.typ {
                Some(MountPointTypeEnum::VOLUME) => mp.name.clone().unwrap_or("-".to_string()),
                Some(_) => mp.source.clone().unwrap_or("-".to_string()),
                None => "-".to_string(),
            };

            let destination = mp.destination.clone().unwrap_or("-".to_string());
            format!("{source} -> {destination}")
        })
        .collect::<Vec<String>>();

    mp.sort_by_key(|v| v.clone());
    mp.join("\n")
}
