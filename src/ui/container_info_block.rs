use std::collections::BTreeMap;

use crate::docker::models::{Mount, PortConfig};
use crate::{event::AppEvent, utils::is_container_running};

use super::common::{RefreshTicker, render_footer, render_scrollbar};
use bollard::secret::{ContainerInspectResponse, ContainerStateStatusEnum};
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
    ticker: RefreshTicker,
}

#[derive(Clone)]
pub struct ContainerData {
    id: String,
    name: String,
    image: String,
    created: String,
    state: ContainerStateStatusEnum,
    ip_address: String,
    start_time: String,
    port_configs: Vec<PortConfig>,
    cmd: Vec<String>,
    entrypoint: Vec<String>,
    env: Vec<String>,
    restart_policy: String,
    volumes: Vec<Mount>,
    labels: BTreeMap<String, String>,
}

impl Default for ContainerData {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            image: String::new(),
            created: String::new(),
            state: ContainerStateStatusEnum::EMPTY,
            ip_address: String::new(),
            start_time: String::new(),
            port_configs: Vec::new(),
            cmd: Vec::new(),
            entrypoint: Vec::new(),
            env: Vec::new(),
            restart_policy: String::new(),
            volumes: Vec::new(),
            labels: BTreeMap::new(),
        }
    }
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

        self.scroll_info.max_horizontal =
            max_horizontal.saturating_sub(info_area.width as usize - 2);
        self.scroll_info.max_vertical = content_lines
            .len()
            .saturating_sub(info_area.height as usize - 2);

        self.render_content(frame, info_area, content_lines);

        self.scroll_info.vertical_state = self
            .scroll_info
            .vertical_state
            .content_length(self.scroll_info.max_vertical)
            .position(self.scroll_info.vertical);

        render_scrollbar(
            frame,
            vertical_scrollbar_area,
            &mut self.scroll_info.vertical_state,
            true,
        );

        self.scroll_info.horizontal_state = self
            .scroll_info
            .horizontal_state
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
            get_footer_text(is_container_running(self.data.state)),
            None,
        );

        Ok(())
    }

    fn tick(&mut self) -> Result<Option<AppEvent>> {
        let event = if self.ticker.should_refresh() {
            Some(AppEvent::UpdateContainerInfo(self.data.id.clone()))
        } else {
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
        ("State: ".to_string(), data.state.to_string()),
        spacer.clone(),
        (
            "CMD: ".to_string(),
            if data.cmd.is_empty() {
                "-".to_string()
            } else {
                data.cmd.join("\n")
            },
        ),
        (
            "Entrypoint: ".to_string(),
            if data.entrypoint.is_empty() {
                "-".to_string()
            } else {
                data.entrypoint.join("\n")
            },
        ),
    ];

    if !data.ip_address.is_empty() {
        lines.extend(vec![
            spacer.clone(),
            ("IP Address: ".to_string(), data.ip_address.clone()),
        ]);
    }

    let port_configs = data.port_configs.iter().map(ToString::to_string).collect();
    if let Some(ports) = format_list(port_configs) {
        lines.extend(vec![
            spacer.clone(),
            ("Port Configs:".to_string(), "".to_string()),
        ]);
        lines.extend(ports);
    }

    let volumes = data.volumes.iter().map(ToString::to_string).collect();
    if let Some(volumes) = format_list(volumes) {
        lines.extend(vec![
            spacer.clone(),
            ("Volumes:".to_string(), "".to_string()),
        ]);
        lines.extend(volumes);
    }

    if let Some(env) = format_list(data.env.clone()) {
        lines.extend(vec![spacer.clone(), ("Env:".to_string(), "".to_string())]);
        lines.extend(env);
    }

    let labels = data
        .labels
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect();
    if let Some(labels) = format_list(labels) {
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

fn format_list(mut entries: Vec<String>) -> Option<Vec<(String, String)>> {
    entries.retain(|e| !e.is_empty());

    if entries.is_empty() {
        return None;
    }

    entries.sort_unstable();
    Some(
        entries
            .iter()
            .map(|d| ("".to_string(), format!(" - {d}")))
            .collect(),
    )
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
        let name = container
            .name
            .as_deref()
            .and_then(|name| name.strip_prefix("/"))
            .map(String::from)
            .unwrap_or_else(|| "NaN".to_string());

        let restart_policy = container
            .host_config
            .as_ref()
            .and_then(|c| c.restart_policy.as_ref())
            .and_then(|c| c.name)
            .map(|name| format!("{name:?}").to_lowercase().replace("_", "-"))
            .unwrap_or_else(|| "-".to_string());

        let mut image = "-".to_string();
        let mut cmd = Vec::new();
        let mut env = Vec::new();
        let mut entrypoint = Vec::new();
        let mut labels = BTreeMap::new();
        if let Some(config) = container.config {
            image = config.image.unwrap_or(image);
            cmd = config.cmd.unwrap_or_default();
            env = config.env.unwrap_or_default();
            entrypoint = config.entrypoint.unwrap_or_default();
            labels = config.labels.unwrap_or_default().into_iter().collect();
        }

        let mut ip_address = "-".to_string();
        let mut port_configs = Vec::new();
        if let Some(network_settings) = container.network_settings {
            ip_address = network_settings.ip_address.unwrap_or(ip_address);
            port_configs = network_settings
                .ports
                .map(|p| PortConfig::from_port_map(&p))
                .unwrap_or_default();
        }

        let mut state = ContainerStateStatusEnum::EMPTY;
        let mut start_time = "-".to_string();
        if let Some(state_info) = container.state {
            state = state_info.status.unwrap_or(state);
            start_time = state_info.started_at.unwrap_or(start_time);
        }

        let volumes = container
            .mounts
            .as_deref()
            .map(Mount::from_mount_points)
            .unwrap_or_default();

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

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::secret::{ContainerConfig, ContainerState};
    use std::collections::HashMap;

    #[test]
    fn from_extracts_typed_state_and_preserves_order() {
        let mut labels = HashMap::new();
        labels.insert("com.example.owner".to_string(), "team-a".to_string());

        let container = ContainerInspectResponse {
            id: Some("abc123".to_string()),
            name: Some("/my-container".to_string()),
            config: Some(ContainerConfig {
                image: Some("alpine".to_string()),
                cmd: Some(vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "sleep 1".to_string(),
                ]),
                env: Some(vec!["A=1".to_string(), "B=2".to_string()]),
                labels: Some(labels),
                ..Default::default()
            }),
            state: Some(ContainerState {
                status: Some(ContainerStateStatusEnum::RUNNING),
                ..Default::default()
            }),
            ..Default::default()
        };

        let data = ContainerData::from(container);

        assert_eq!(data.state, ContainerStateStatusEnum::RUNNING);
        assert_eq!(data.cmd, vec!["sh", "-c", "sleep 1"]);
        assert_eq!(data.env, vec!["A=1", "B=2"]);
        assert_eq!(
            data.labels.get("com.example.owner"),
            Some(&"team-a".to_string())
        );
    }

    #[test]
    fn from_empty_inspect_yields_empty_collections() {
        let data = ContainerData::from(ContainerInspectResponse::default());

        assert_eq!(data.state, ContainerStateStatusEnum::EMPTY);
        assert!(data.cmd.is_empty());
        assert!(data.entrypoint.is_empty());
        assert!(data.env.is_empty());
        assert!(data.labels.is_empty());
        assert!(data.port_configs.is_empty());
        assert!(data.volumes.is_empty());
    }

    #[test]
    fn get_content_as_lines_drops_empty_optional_sections() {
        let data = ContainerData::default();
        let lines: Vec<String> = get_content_as_lines(&data)
            .iter()
            .map(|l| l.to_string())
            .collect();

        // CMD/Entrypoint are "-" when empty and must be filtered out entirely.
        assert!(!lines.iter().any(|l| l.starts_with("CMD: ")));
        assert!(!lines.iter().any(|l| l.starts_with("Entrypoint: ")));

        // No optional sections should render when their data is empty.
        assert!(!lines.iter().any(|l| l.starts_with("Env:")));
        assert!(!lines.iter().any(|l| l.starts_with("Labels:")));
    }
}
