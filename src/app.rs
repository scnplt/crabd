use crate::docker::client::{DockerApi, DockerClient};
use crate::docker::error::{DockerError, DockerResult};
use crate::event::{AppEvent, DockerOutcome, Event, EventHandler, ResourceKind};
use crate::ui::container_info_block::{ContainerData, ContainerInfoBlock};
use crate::ui::container_logs_block::ContainerLogsBlock;
use crate::ui::container_table::{ContainerTable, ContainerTableRow};
use crate::ui::image_table::{ImageTable, ImageTableRow};
use crate::ui::info_block::ScrollableInfoBlock;
use crate::ui::network_table::{NetworkTable, NetworkTableRow};
use crate::ui::resource_table::{ResourceRow, ResourceTable};
use crate::ui::volume_table::{VolumeTable, VolumeTableRow};
use color_eyre::eyre::Result;
use futures::StreamExt;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::palette::tailwind;
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;
use ratatui::widgets::Tabs;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};
use std::future::Future;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, FromRepr};
use tokio::task::JoinHandle;

pub struct App<C: DockerApi> {
    running: bool,
    events: EventHandler,
    docker_client: C,
    selected_tab: SelectedTab,
    container_table: ContainerTable,
    overlay: Option<Overlay>,
    logs_task: Option<JoinHandle<()>>,
    logs_session: u64,
    /// Set when the log view was opened from the details overlay, so `Back`
    /// returns to the details of this container instead of the table.
    logs_return_to_details: Option<String>,
    volume_table: VolumeTable,
    network_table: NetworkTable,
    image_table: ImageTable,
    pending: PendingRefreshes,
    details_requested: bool,
    dirty: bool,
}

/// At most one full-screen overlay can be open over the tab view at a time.
enum Overlay {
    Info(Box<dyn ScrollableInfoBlock<Data = ContainerData>>),
    Logs(ContainerLogsBlock),
}

/// Tracks in-flight list requests so `App` never has more than one outstanding
/// refresh per resource kind at a time.
#[derive(Default)]
struct PendingRefreshes {
    containers: bool,
    volumes: bool,
    networks: bool,
    images: bool,
    container_info: bool,
}

impl App<DockerClient> {
    pub fn new() -> Result<Self> {
        Ok(Self {
            running: true,
            events: EventHandler::new(),
            docker_client: DockerClient::new()?,
            selected_tab: SelectedTab::default(),
            container_table: ContainerTable::default(),
            overlay: None,
            logs_task: None,
            logs_session: 0,
            logs_return_to_details: None,
            volume_table: VolumeTable::default(),
            network_table: NetworkTable::default(),
            image_table: ImageTable::default(),
            pending: PendingRefreshes::default(),
            details_requested: false,
            dirty: true,
        })
    }
}

impl<C: DockerApi> App<C> {
    /// Test-only constructor; builds an `App` from a caller-provided `DockerApi`
    /// implementation, using the event handler's test constructor so no crossterm
    /// reader task is spawned.
    #[cfg(test)]
    fn with_client(docker_client: C) -> Self {
        Self {
            running: true,
            events: EventHandler::new_without_reader(),
            docker_client,
            selected_tab: SelectedTab::default(),
            container_table: ContainerTable::default(),
            overlay: None,
            logs_task: None,
            logs_session: 0,
            logs_return_to_details: None,
            volume_table: VolumeTable::default(),
            network_table: NetworkTable::default(),
            image_table: ImageTable::default(),
            pending: PendingRefreshes::default(),
            details_requested: false,
            dirty: true,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.request_containers();

        while self.running {
            if self.dirty {
                terminal.draw(|frame| self.draw(frame, frame.area()))?;
                self.dirty = false;
            }
            self.process_next_event().await?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        use Constraint::{Length, Min};
        let vertical = Layout::vertical([Length(1), Length(1), Min(0)]);
        let [header_area, _, inner_area] = vertical.areas(area);

        let header_horizontal = Layout::horizontal([Min(0), Length(6)]);
        let [tabs_area, title_area] = header_horizontal.areas(header_area);

        match self.overlay.as_mut() {
            Some(Overlay::Info(block)) => {
                let _ = block.draw(frame, area);
            }
            Some(Overlay::Logs(block)) => {
                let _ = block.draw(frame, area);
            }
            None => {
                render_title(frame, title_area);
                self.render_tabs(frame, tabs_area);
                let _ = self.render_selected_tab(frame, inner_area);
            }
        }
    }

    fn render_tabs(&mut self, frame: &mut Frame, area: Rect) {
        let titles = SelectedTab::iter().map(SelectedTab::title);
        let hightlight_style = (Color::default(), tailwind::SLATE.c700);
        let selected_tab_index = self.selected_tab as usize;

        let tabs = Tabs::new(titles)
            .highlight_style(hightlight_style)
            .select(selected_tab_index)
            .padding("", "")
            .divider(" ");

        frame.render_widget(tabs, area);
    }

    fn render_selected_tab(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        match self.selected_tab {
            SelectedTab::Containers => self.container_table.draw(frame, area)?,
            SelectedTab::Volumes => self.volume_table.draw(frame, area)?,
            SelectedTab::Networks => self.network_table.draw(frame, area)?,
            SelectedTab::Images => self.image_table.draw(frame, area)?,
        }
        Ok(())
    }

    async fn process_next_event(&mut self) -> Result<()> {
        match self.events.next().await? {
            Event::Tick => {
                if let Some(event) = self.tick()? {
                    self.events.send(event)
                }
            }
            Event::Crossterm(key_event) => {
                self.dirty = true;
                if let Some(event) = self.handle_key_event(key_event)? {
                    self.events.send(event);
                }
            }
            Event::Resize => {
                self.dirty = true;
            }
            Event::Docker(outcome) => self.apply_docker_outcome(outcome),
            Event::App(app_event) => {
                if matches!(
                    app_event,
                    AppEvent::Back
                        | AppEvent::RestartContainer(_)
                        | AppEvent::StopContainer(_)
                        | AppEvent::KillContainer(_)
                        | AppEvent::RemoveContainer(_)
                        | AppEvent::RemoveVolume(..)
                        | AppEvent::RemoveNetwork(_)
                        | AppEvent::RemoveImage(..)
                ) {
                    self.dirty = true;
                }

                match app_event {
                    AppEvent::Quit => self.quit(),
                    AppEvent::UpdateContainers => self.request_containers(),
                    AppEvent::UpdateContainerInfo(id) => self.request_container_details(id),
                    AppEvent::RestartContainer(id) => self.request_restart_container(id),
                    AppEvent::StopContainer(id) => self.request_stop_container(id),
                    AppEvent::KillContainer(id) => self.request_kill_container(id),
                    AppEvent::RemoveContainer(id) => self.request_remove_container(id),
                    AppEvent::GoToContainerDetails(id) => self.request_container_details_open(id),
                    AppEvent::GoToContainerLogs { id, name } => self.open_logs(id, name),
                    AppEvent::UpdateVolumes => self.request_volumes(),
                    AppEvent::RemoveVolume(name, force) => self.request_remove_volume(name, force),
                    AppEvent::UpdateNetworks => self.request_networks(),
                    AppEvent::RemoveNetwork(name) => self.request_remove_network(name),
                    AppEvent::UpdateImages => self.request_images(),
                    AppEvent::RemoveImage(id, force) => self.request_remove_image(id, force),
                    AppEvent::Back => {
                        self.details_requested = false;
                        let return_to_details = match self.overlay {
                            Some(Overlay::Logs(_)) => self.logs_return_to_details.take(),
                            _ => None,
                        };
                        self.close_logs();
                        self.overlay = None;
                        if let Some(id) = return_to_details {
                            self.request_container_details_open(id);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if key_event.code == KeyCode::Char('c') && key_event.modifiers == KeyModifiers::CONTROL {
            return Ok(Some(AppEvent::Quit));
        }

        match self.overlay.as_mut() {
            Some(Overlay::Info(block)) => return block.handle_key_event(key_event),
            Some(Overlay::Logs(block)) => return block.handle_key_event(key_event),
            None => {}
        }

        let event = match key_event.code {
            KeyCode::Right | KeyCode::Char('l' | 'L') => {
                self.next_tab();
                None
            }
            KeyCode::Left | KeyCode::Char('h' | 'H') => {
                self.previous_tab();
                None
            }
            _ => match self.selected_tab {
                SelectedTab::Containers => self.container_table.handle_key_event(key_event)?,
                SelectedTab::Volumes => self.volume_table.handle_key_event(key_event)?,
                SelectedTab::Networks => self.network_table.handle_key_event(key_event)?,
                SelectedTab::Images => self.image_table.handle_key_event(key_event)?,
            },
        };

        Ok(event)
    }

    fn next_tab(&mut self) {
        self.selected_tab = self.selected_tab.next()
    }

    fn previous_tab(&mut self) {
        self.selected_tab = self.selected_tab.previous()
    }

    fn tick(&mut self) -> Result<Option<AppEvent>> {
        match self.overlay.as_mut() {
            Some(Overlay::Info(block)) => return block.tick(),
            Some(Overlay::Logs(_)) => return Ok(None),
            None => {}
        }

        let event = match self.selected_tab {
            SelectedTab::Containers => self.container_table.tick()?,
            SelectedTab::Volumes => self.volume_table.tick()?,
            SelectedTab::Networks => self.network_table.tick()?,
            SelectedTab::Images => self.image_table.tick()?,
        };

        Ok(event)
    }

    fn quit(&mut self) {
        self.close_logs();
        self.running = false;
    }

    fn report_err(&mut self, tab: SelectedTab, err: &DockerError) {
        let changed = match tab {
            SelectedTab::Containers => self.container_table.show_err(err),
            SelectedTab::Volumes => self.volume_table.show_err(err),
            SelectedTab::Networks => self.network_table.show_err(err),
            SelectedTab::Images => self.image_table.show_err(err),
        };
        self.dirty |= changed;
    }

    /// Unwraps a Docker result, reporting failures into `tab`'s footer.
    fn ok_or_report<T>(&mut self, tab: SelectedTab, result: DockerResult<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                self.report_err(tab, &err);
                None
            }
        }
    }

    /// Maps a Docker outcome's resource kind back to the tab that owns it.
    fn tab_of(resource: ResourceKind) -> SelectedTab {
        match resource {
            ResourceKind::Containers => SelectedTab::Containers,
            ResourceKind::Volumes => SelectedTab::Volumes,
            ResourceKind::Networks => SelectedTab::Networks,
            ResourceKind::Images => SelectedTab::Images,
        }
    }

    fn begin_action(&mut self, tab: SelectedTab) {
        match tab {
            SelectedTab::Containers => self.container_table.begin_pending_op(),
            SelectedTab::Volumes => self.volume_table.begin_pending_op(),
            SelectedTab::Networks => self.network_table.begin_pending_op(),
            SelectedTab::Images => self.image_table.begin_pending_op(),
        }
        self.dirty = true;
    }

    fn end_action(&mut self, tab: SelectedTab) {
        match tab {
            SelectedTab::Containers => self.container_table.end_pending_op(),
            SelectedTab::Volumes => self.volume_table.end_pending_op(),
            SelectedTab::Networks => self.network_table.end_pending_op(),
            SelectedTab::Images => self.image_table.end_pending_op(),
        }
        self.dirty = true;
    }

    /// Runs a Docker operation off the event loop; its outcome comes back as `Event::Docker`.
    fn spawn_docker<F, Fut>(&self, op: F)
    where
        F: FnOnce(C) -> Fut + Send + 'static,
        Fut: Future<Output = DockerOutcome> + Send + 'static,
    {
        let client = self.docker_client.clone();
        let sender = self.events.sender();
        tokio::spawn(async move {
            let _ = sender.send(Event::Docker(op(client).await));
        });
    }

    fn request_containers(&mut self) {
        if self.pending.containers {
            return;
        }
        self.pending.containers = true;
        self.spawn_docker(
            |c| async move { DockerOutcome::ContainersListed(c.list_containers().await) },
        );
    }

    fn request_volumes(&mut self) {
        if self.pending.volumes {
            return;
        }
        self.pending.volumes = true;
        self.spawn_docker(|c| async move { DockerOutcome::VolumesListed(c.list_volumes().await) });
    }

    fn request_networks(&mut self) {
        if self.pending.networks {
            return;
        }
        self.pending.networks = true;
        self.spawn_docker(
            |c| async move { DockerOutcome::NetworksListed(c.list_networks().await) },
        );
    }

    fn request_images(&mut self) {
        if self.pending.images {
            return;
        }
        self.pending.images = true;
        self.spawn_docker(|c| async move { DockerOutcome::ImagesListed(c.list_images().await) });
    }

    fn request_container_details(&mut self, id: String) {
        if self.pending.container_info {
            return;
        }
        self.pending.container_info = true;
        self.spawn_docker(|c| async move {
            let result = c.inspect_container(&id).await.map(Box::new);
            DockerOutcome::ContainerInspected {
                open_details: false,
                result,
            }
        });
    }

    fn request_container_details_open(&mut self, id: String) {
        self.details_requested = true;
        self.spawn_docker(|c| async move {
            let result = c.inspect_container(&id).await.map(Box::new);
            DockerOutcome::ContainerInspected {
                open_details: true,
                result,
            }
        });
    }

    /// Opens the full-screen log view for `id` and starts a background task that
    /// pumps the log stream into the event channel as `DockerOutcome::ContainerLogChunk`
    /// batches. `logs_session` tags every chunk so events from a stream aborted by a
    /// later `close_logs()` (still queued in the unbounded channel) are dropped on arrival.
    fn open_logs(&mut self, id: String, name: String) {
        self.logs_return_to_details =
            matches!(self.overlay, Some(Overlay::Info(_))).then(|| id.clone());
        self.close_logs();
        self.logs_session += 1;
        let session = self.logs_session;
        self.overlay = Some(Overlay::Logs(ContainerLogsBlock::new(id.clone(), name)));
        self.dirty = true;

        let client = self.docker_client.clone();
        let sender = self.events.sender();
        self.logs_task = Some(tokio::spawn(async move {
            let mut chunks = Box::pin(client.container_logs(&id, 500).ready_chunks(64));
            while let Some(batch) = chunks.next().await {
                let mut lines = Vec::new();
                let mut stream_error = None;
                for result in batch {
                    match result {
                        Ok(chunk) => lines.extend(split_lines(chunk)),
                        Err(e) => {
                            stream_error = Some(e);
                            break;
                        }
                    }
                }
                if !lines.is_empty() {
                    let _ = sender.send(Event::Docker(DockerOutcome::ContainerLogChunk {
                        session,
                        lines,
                    }));
                }
                if let Some(error) = stream_error {
                    let _ = sender.send(Event::Docker(DockerOutcome::ContainerLogsEnded {
                        session,
                        error: Some(error),
                    }));
                    return;
                }
            }
            let _ = sender.send(Event::Docker(DockerOutcome::ContainerLogsEnded {
                session,
                error: None,
            }));
        }));
    }

    /// Aborts any in-flight log stream task and invalidates its already-queued events.
    fn close_logs(&mut self) {
        if let Some(task) = self.logs_task.take() {
            task.abort();
        }
        self.logs_session += 1;
    }

    fn request_restart_container(&mut self, id: String) {
        self.begin_action(SelectedTab::Containers);
        self.spawn_docker(|c| async move {
            DockerOutcome::ActionCompleted {
                resource: ResourceKind::Containers,
                result: c.restart_container(&id).await,
            }
        });
    }

    fn request_stop_container(&mut self, id: String) {
        self.begin_action(SelectedTab::Containers);
        self.spawn_docker(|c| async move {
            DockerOutcome::ActionCompleted {
                resource: ResourceKind::Containers,
                result: c.stop_container(&id).await,
            }
        });
    }

    fn request_kill_container(&mut self, id: String) {
        self.begin_action(SelectedTab::Containers);
        self.spawn_docker(|c| async move {
            DockerOutcome::ActionCompleted {
                resource: ResourceKind::Containers,
                result: c.kill_container(&id).await,
            }
        });
    }

    fn request_remove_container(&mut self, id: String) {
        self.begin_action(SelectedTab::Containers);
        self.spawn_docker(|c| async move {
            DockerOutcome::ActionCompleted {
                resource: ResourceKind::Containers,
                result: c.remove_container(&id).await,
            }
        });
    }

    fn request_remove_volume(&mut self, name: String, force: bool) {
        self.begin_action(SelectedTab::Volumes);
        self.spawn_docker(move |c| async move {
            DockerOutcome::ActionCompleted {
                resource: ResourceKind::Volumes,
                result: c.remove_volume(&name, force).await,
            }
        });
    }

    fn request_remove_network(&mut self, name: String) {
        self.begin_action(SelectedTab::Networks);
        self.spawn_docker(|c| async move {
            DockerOutcome::ActionCompleted {
                resource: ResourceKind::Networks,
                result: c.remove_network(&name).await,
            }
        });
    }

    fn request_remove_image(&mut self, id: String, force: bool) {
        self.begin_action(SelectedTab::Images);
        self.spawn_docker(move |c| async move {
            DockerOutcome::ActionCompleted {
                resource: ResourceKind::Images,
                result: c.remove_image(&id, force).await,
            }
        });
    }

    fn apply_docker_outcome(&mut self, outcome: DockerOutcome) {
        match outcome {
            DockerOutcome::ContainersListed(result) => {
                self.pending.containers = false;
                if let Some(list) = self.ok_or_report(SelectedTab::Containers, result) {
                    self.dirty |= self
                        .container_table
                        .update_with_items(ContainerTableRow::from_list(list));
                }
            }
            DockerOutcome::VolumesListed(result) => {
                self.pending.volumes = false;
                if let Some(response) = self.ok_or_report(SelectedTab::Volumes, result)
                    && let Some(volumes) = response.volumes
                {
                    self.dirty |= self
                        .volume_table
                        .update_with_items(VolumeTableRow::from_list(volumes));
                }
            }
            DockerOutcome::NetworksListed(result) => {
                self.pending.networks = false;
                if let Some(list) = self.ok_or_report(SelectedTab::Networks, result) {
                    self.dirty |= self
                        .network_table
                        .update_with_items(NetworkTableRow::from_list(list));
                }
            }
            DockerOutcome::ImagesListed(result) => {
                self.pending.images = false;
                if let Some(list) = self.ok_or_report(SelectedTab::Images, result) {
                    self.dirty |= self
                        .image_table
                        .update_with_items(ImageTableRow::from_list(list));
                }
            }
            DockerOutcome::ContainerInspected {
                open_details,
                result,
            } => {
                if !open_details {
                    self.pending.container_info = false;
                }
                let Some(data) = self
                    .ok_or_report(SelectedTab::Containers, result)
                    .map(|r| ContainerData::from(*r))
                else {
                    return;
                };
                if open_details {
                    if !self.details_requested {
                        return;
                    }
                    let mut container_info_block = ContainerInfoBlock::default();
                    container_info_block.update_data(data);
                    self.overlay = Some(Overlay::Info(Box::new(container_info_block)));
                    self.dirty = true;
                } else if let Some(Overlay::Info(block)) = self.overlay.as_mut() {
                    self.dirty |= block.update_data(data);
                }
            }
            DockerOutcome::ActionCompleted { resource, result } => {
                let tab = Self::tab_of(resource);
                self.end_action(tab);
                self.ok_or_report(tab, result);
            }
            DockerOutcome::ContainerLogChunk { session, lines } => {
                if session != self.logs_session {
                    return;
                }
                if let Some(Overlay::Logs(block)) = self.overlay.as_mut() {
                    self.dirty |= block.push_lines(lines);
                }
            }
            DockerOutcome::ContainerLogsEnded { session, error } => {
                if session != self.logs_session {
                    return;
                }
                if let Some(Overlay::Logs(block)) = self.overlay.as_mut() {
                    self.dirty |= block.mark_ended(error);
                }
            }
        }
    }
}

fn render_title(frame: &mut Frame, area: Rect) {
    let title = " crabd".bold();
    frame.render_widget(title, area);
}

/// Splits a decoded log chunk into individual lines. A frame without a trailing
/// newline becomes its own line (no carry-over buffer across chunks).
fn split_lines(chunk: String) -> Vec<String> {
    if chunk.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<String> = chunk.split('\n').map(str::to_string).collect();
    if chunk.ends_with('\n') {
        lines.pop();
    }
    lines
}

#[derive(Default, Display, FromRepr, EnumIter, Clone, Copy)]
enum SelectedTab {
    #[default]
    #[strum(to_string = "Containers")]
    Containers,

    #[strum(to_string = "Volumes")]
    Volumes,

    #[strum(to_string = "Networks")]
    Networks,

    #[strum(to_string = "Images")]
    Images,
}

impl SelectedTab {
    fn title(self) -> Line<'static> {
        format!("  {self}  ")
            .fg(tailwind::SLATE.c200)
            .bg(tailwind::SLATE.c900)
            .into()
    }

    fn next(self) -> Self {
        let current_index = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(self)
    }

    fn previous(self) -> Self {
        let current_index = self as usize;
        let previous_index = current_index.saturating_sub(1);
        Self::from_repr(previous_index).unwrap_or(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::secret::{
        ContainerInspectResponse, ContainerStateStatusEnum, ContainerSummary, ImageSummary,
        Network, Volume, VolumeListResponse,
    };
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockDockerClient {
        containers: Vec<ContainerSummary>,
        volumes: Vec<Volume>,
        networks: Vec<Network>,
        images: Vec<ImageSummary>,
        inspect: Option<ContainerInspectResponse>,
        log_lines: Vec<String>,
        fail_with: Option<DockerError>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl MockDockerClient {
        fn record(&self, call: impl Into<String>) {
            self.calls.lock().unwrap().push(call.into());
        }
    }

    impl DockerApi for MockDockerClient {
        async fn list_containers(&self) -> DockerResult<Vec<ContainerSummary>> {
            self.record("list_containers");
            if let Some(e) = &self.fail_with {
                return Err(e.clone());
            }
            Ok(self.containers.clone())
        }

        async fn stop_container(&self, container_id: &str) -> DockerResult<()> {
            self.record(format!("stop_container:{container_id}"));
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }

        async fn restart_container(&self, container_id: &str) -> DockerResult<()> {
            self.record(format!("restart_container:{container_id}"));
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }

        async fn kill_container(&self, container_id: &str) -> DockerResult<()> {
            self.record(format!("kill_container:{container_id}"));
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }

        async fn remove_container(&self, container_id: &str) -> DockerResult<()> {
            self.record(format!("remove_container:{container_id}"));
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }

        async fn inspect_container(
            &self,
            container_id: &str,
        ) -> DockerResult<ContainerInspectResponse> {
            self.record(format!("inspect_container:{container_id}"));
            if let Some(e) = &self.fail_with {
                return Err(e.clone());
            }
            self.inspect.clone().ok_or_else(|| DockerError::Other {
                message: "no inspect data configured".to_string(),
            })
        }

        async fn list_volumes(&self) -> DockerResult<VolumeListResponse> {
            self.record("list_volumes");
            if let Some(e) = &self.fail_with {
                return Err(e.clone());
            }
            Ok(VolumeListResponse {
                volumes: Some(self.volumes.clone()),
                ..Default::default()
            })
        }

        async fn remove_volume(&self, name: &str, force: bool) -> DockerResult<()> {
            self.record(format!("remove_volume:{name}:{force}"));
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }

        async fn list_networks(&self) -> DockerResult<Vec<Network>> {
            self.record("list_networks");
            if let Some(e) = &self.fail_with {
                return Err(e.clone());
            }
            Ok(self.networks.clone())
        }

        async fn remove_network(&self, name: &str) -> DockerResult<()> {
            self.record(format!("remove_network:{name}"));
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }

        async fn list_images(&self) -> DockerResult<Vec<ImageSummary>> {
            self.record("list_images");
            if let Some(e) = &self.fail_with {
                return Err(e.clone());
            }
            Ok(self.images.clone())
        }

        async fn remove_image(&self, id: &str, force: bool) -> DockerResult<()> {
            self.record(format!("remove_image:{id}:{force}"));
            match &self.fail_with {
                Some(e) => Err(e.clone()),
                None => Ok(()),
            }
        }

        fn container_logs(
            &self,
            container_id: &str,
            tail: usize,
        ) -> impl futures::Stream<Item = DockerResult<String>> + Send {
            self.record(format!("container_logs:{container_id}:{tail}"));
            let items: Vec<DockerResult<String>> = if let Some(e) = &self.fail_with {
                vec![Err(e.clone())]
            } else {
                self.log_lines.iter().cloned().map(Ok).collect()
            };
            futures::stream::iter(items)
        }
    }

    fn container_summary(id: &str) -> ContainerSummary {
        ContainerSummary {
            id: Some(id.to_string()),
            names: Some(vec![format!("/{id}")]),
            state: Some("running".to_string()),
            ..Default::default()
        }
    }

    fn volume(name: &str) -> Volume {
        Volume {
            name: name.to_string(),
            driver: "local".to_string(),
            ..Default::default()
        }
    }

    fn network(id: &str, name: &str) -> Network {
        Network {
            id: Some(id.to_string()),
            name: Some(name.to_string()),
            driver: Some("bridge".to_string()),
            ..Default::default()
        }
    }

    fn image(id: &str) -> ImageSummary {
        ImageSummary {
            id: format!("sha256:{id}"),
            ..Default::default()
        }
    }

    #[test]
    fn update_containers_populates_table() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ContainersListed(Ok(vec![
            container_summary("a"),
            container_summary("b"),
        ])));

        assert_eq!(app.container_table.table_info().items.len(), 2);
    }

    #[test]
    fn update_volumes_populates_table() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::VolumesListed(Ok(VolumeListResponse {
            volumes: Some(vec![volume("a"), volume("b")]),
            ..Default::default()
        })));

        assert_eq!(app.volume_table.table_info().items.len(), 2);
    }

    #[test]
    fn update_networks_populates_table() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::NetworksListed(Ok(vec![
            network("111111111111", "a"),
            network("222222222222", "b"),
        ])));

        assert_eq!(app.network_table.table_info().items.len(), 2);
    }

    #[test]
    fn update_images_populates_table() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ImagesListed(Ok(vec![
            image("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd"),
            image("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abce"),
        ])));

        assert_eq!(app.image_table.table_info().items.len(), 2);
    }

    #[test]
    fn restart_container_error_is_shown_in_footer() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ActionCompleted {
            resource: ResourceKind::Containers,
            result: Err(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
        });

        let err = app.container_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[test]
    fn stop_container_error_is_shown_in_footer() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ActionCompleted {
            resource: ResourceKind::Containers,
            result: Err(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
        });

        let err = app.container_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[test]
    fn kill_container_error_is_shown_in_footer() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ActionCompleted {
            resource: ResourceKind::Containers,
            result: Err(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
        });

        let err = app.container_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[test]
    fn remove_volume_error_is_shown_in_footer() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ActionCompleted {
            resource: ResourceKind::Volumes,
            result: Err(DockerError::Conflict {
                message: "volume is in use".to_string(),
            }),
        });

        let err = app.volume_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: volume is in use");
    }

    #[test]
    fn remove_network_error_is_shown_in_footer() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ActionCompleted {
            resource: ResourceKind::Networks,
            result: Err(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
        });

        let err = app.network_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[test]
    fn remove_image_error_is_shown_in_footer() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ActionCompleted {
            resource: ResourceKind::Images,
            result: Err(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
        });

        let err = app.image_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[test]
    fn list_failure_shows_error_on_owning_tab() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);
        assert_eq!(app.selected_tab as usize, SelectedTab::Containers as usize);

        app.apply_docker_outcome(DockerOutcome::ImagesListed(Err(
            DockerError::DaemonUnreachable {
                details: "connection refused".to_string(),
            },
        )));

        assert!(app.image_table.table_info().err.is_some());
        assert!(app.container_table.table_info().err.is_none());
    }

    #[test]
    fn list_failure_keeps_previous_items() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ContainersListed(Err(
            DockerError::DaemonUnreachable {
                details: "connection refused".to_string(),
            },
        )));

        assert_eq!(app.container_table.table_info().items.len(), 0);
        assert!(app.container_table.table_info().err.is_some());
    }

    #[tokio::test]
    async fn go_to_container_info_opens_and_back_closes() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.request_container_details_open("container-id".to_string());
        app.apply_docker_outcome(DockerOutcome::ContainerInspected {
            open_details: true,
            result: Ok(Box::new(ContainerInspectResponse {
                id: Some("container-id".to_string()),
                ..Default::default()
            })),
        });
        assert!(matches!(app.overlay, Some(Overlay::Info(_))));

        let event = app.handle_key_event(KeyEvent::from(KeyCode::Esc)).unwrap();
        assert!(matches!(event, Some(AppEvent::Back)));
    }

    #[tokio::test]
    async fn key_events_route_to_info_block_when_open() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.request_container_details_open("container-id".to_string());
        app.apply_docker_outcome(DockerOutcome::ContainerInspected {
            open_details: true,
            result: Ok(Box::new(ContainerInspectResponse {
                id: Some("container-id".to_string()),
                ..Default::default()
            })),
        });

        let tab_before = app.selected_tab as usize;
        app.handle_key_event(KeyEvent::from(KeyCode::Char('t')))
            .unwrap();
        assert_eq!(app.selected_tab as usize, tab_before);
    }

    #[tokio::test]
    async fn request_containers_dispatches_to_the_client() {
        let mock = MockDockerClient::default();
        let calls = mock.calls.clone();
        let mut app = App::with_client(mock);

        app.request_containers();

        let event = app.events.next().await.unwrap();
        assert!(matches!(
            event,
            Event::Docker(DockerOutcome::ContainersListed(Ok(_)))
        ));
        assert_eq!(*calls.lock().unwrap(), vec!["list_containers".to_string()]);
    }

    #[tokio::test]
    async fn duplicate_refresh_is_dropped_while_one_is_in_flight() {
        let mock = MockDockerClient::default();
        let calls = mock.calls.clone();
        let mut app = App::with_client(mock);

        app.request_containers();
        app.request_containers();

        let _ = app.events.next().await.unwrap();
        assert!(app.events.try_next().is_none());
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn refresh_is_allowed_again_after_the_outcome_is_applied() {
        let mock = MockDockerClient::default();
        let calls = mock.calls.clone();
        let mut app = App::with_client(mock);

        app.request_containers();
        let event = app.events.next().await.unwrap();
        if let Event::Docker(outcome) = event {
            app.apply_docker_outcome(outcome);
        }

        app.request_containers();
        let _ = app.events.next().await.unwrap();

        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn actions_are_not_deduplicated() {
        let mock = MockDockerClient::default();
        let calls = mock.calls.clone();
        let mut app = App::with_client(mock);

        app.request_stop_container("container-a".to_string());
        app.request_stop_container("container-b".to_string());

        let _ = app.events.next().await.unwrap();
        let _ = app.events.next().await.unwrap();

        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn action_marks_and_clears_the_pending_indicator() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.request_stop_container("container-id".to_string());
        assert_eq!(app.container_table.table_info().pending_ops, 1);

        app.apply_docker_outcome(DockerOutcome::ActionCompleted {
            resource: ResourceKind::Containers,
            result: Ok(()),
        });
        assert_eq!(app.container_table.table_info().pending_ops, 0);
    }

    #[tokio::test]
    async fn back_prevents_a_late_details_open() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.request_container_details_open("container-id".to_string());
        app.details_requested = false;
        app.overlay = None;

        app.apply_docker_outcome(DockerOutcome::ContainerInspected {
            open_details: true,
            result: Ok(Box::new(ContainerInspectResponse {
                id: Some("container-id".to_string()),
                ..Default::default()
            })),
        });

        assert!(app.overlay.is_none());
    }

    #[tokio::test]
    async fn stop_container_failure_reaches_the_footer_through_the_channel() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.request_stop_container("container-id".to_string());
        app.process_next_event().await.unwrap();

        let err = app.container_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[test]
    fn ctrl_c_quits() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        let event = app
            .handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
            .unwrap();
        assert!(matches!(event, Some(AppEvent::Quit)));

        app.quit();
        assert!(!app.running);
    }

    #[test]
    fn arrow_and_hjkl_change_tabs() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        assert_eq!(app.selected_tab as usize, SelectedTab::Containers as usize);

        app.handle_key_event(KeyEvent::from(KeyCode::Right))
            .unwrap();
        assert_eq!(app.selected_tab as usize, SelectedTab::Volumes as usize);

        app.handle_key_event(KeyEvent::from(KeyCode::Char('l')))
            .unwrap();
        assert_eq!(app.selected_tab as usize, SelectedTab::Networks as usize);

        app.handle_key_event(KeyEvent::from(KeyCode::Left)).unwrap();
        assert_eq!(app.selected_tab as usize, SelectedTab::Volumes as usize);

        app.handle_key_event(KeyEvent::from(KeyCode::Char('h')))
            .unwrap();
        assert_eq!(app.selected_tab as usize, SelectedTab::Containers as usize);

        // Saturates at the low end.
        app.handle_key_event(KeyEvent::from(KeyCode::Left)).unwrap();
        assert_eq!(app.selected_tab as usize, SelectedTab::Containers as usize);

        // Saturates at the high end.
        for _ in 0..5 {
            app.handle_key_event(KeyEvent::from(KeyCode::Right))
                .unwrap();
        }
        assert_eq!(app.selected_tab as usize, SelectedTab::Images as usize);
    }

    #[test]
    fn selected_tab_next_and_previous_saturate() {
        assert_eq!(SelectedTab::Containers.previous() as usize, 0);
        assert_eq!(
            SelectedTab::Containers.next() as usize,
            SelectedTab::Volumes as usize
        );
        assert_eq!(
            SelectedTab::Volumes.next() as usize,
            SelectedTab::Networks as usize
        );
        assert_eq!(
            SelectedTab::Networks.next() as usize,
            SelectedTab::Images as usize
        );
        assert_eq!(
            SelectedTab::Images.next() as usize,
            SelectedTab::Images as usize
        );
        assert_eq!(
            SelectedTab::Volumes.previous() as usize,
            SelectedTab::Containers as usize
        );
    }

    #[test]
    fn selected_tab_title_renders_padded_name() {
        assert_eq!(
            SelectedTab::Containers.title().to_string(),
            "  Containers  "
        );
    }

    #[test]
    fn app_starts_dirty() {
        let mock = MockDockerClient::default();
        let app = App::with_client(mock);
        assert!(app.dirty);
    }

    #[tokio::test]
    async fn tick_does_not_mark_dirty() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);
        app.dirty = false;

        app.events.sender().send(Event::Tick).unwrap();
        app.process_next_event().await.unwrap();

        assert!(!app.dirty);
    }

    #[tokio::test]
    async fn key_event_marks_dirty() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);
        app.dirty = false;

        app.events
            .sender()
            .send(Event::Crossterm(KeyEvent::from(KeyCode::Right)))
            .unwrap();
        app.process_next_event().await.unwrap();

        assert!(app.dirty);
    }

    #[tokio::test]
    async fn resize_event_marks_dirty() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);
        app.dirty = false;

        app.events.sender().send(Event::Resize).unwrap();
        app.process_next_event().await.unwrap();

        assert!(app.dirty);
        assert!(app.running);
    }

    #[test]
    fn unchanged_container_list_does_not_mark_dirty() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ContainersListed(Ok(vec![
            container_summary("a"),
        ])));
        app.dirty = false;

        app.apply_docker_outcome(DockerOutcome::ContainersListed(Ok(vec![
            container_summary("a"),
        ])));
        assert!(!app.dirty);

        app.apply_docker_outcome(DockerOutcome::ContainersListed(Ok(vec![
            container_summary("a"),
            container_summary("b"),
        ])));
        assert!(app.dirty);
    }

    #[test]
    fn list_error_marks_dirty_once() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ContainersListed(Err(
            DockerError::DaemonUnreachable {
                details: "connection refused".to_string(),
            },
        )));
        assert!(app.dirty);
        app.dirty = false;

        app.apply_docker_outcome(DockerOutcome::ContainersListed(Err(
            DockerError::DaemonUnreachable {
                details: "connection refused".to_string(),
            },
        )));
        assert!(!app.dirty);
    }

    #[tokio::test]
    async fn action_request_and_completion_mark_dirty() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);
        app.dirty = false;

        app.request_stop_container("container-id".to_string());
        assert!(app.dirty);
        app.dirty = false;

        app.apply_docker_outcome(DockerOutcome::ActionCompleted {
            resource: ResourceKind::Containers,
            result: Ok(()),
        });
        assert!(app.dirty);
    }

    #[tokio::test]
    async fn container_info_refresh_with_identical_data_does_not_mark_dirty() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.request_container_details_open("container-id".to_string());
        let inspect = ContainerInspectResponse {
            id: Some("container-id".to_string()),
            state: Some(bollard::secret::ContainerState {
                status: Some(ContainerStateStatusEnum::RUNNING),
                ..Default::default()
            }),
            ..Default::default()
        };
        app.apply_docker_outcome(DockerOutcome::ContainerInspected {
            open_details: true,
            result: Ok(Box::new(inspect.clone())),
        });
        assert!(matches!(app.overlay, Some(Overlay::Info(_))));
        app.dirty = false;

        app.apply_docker_outcome(DockerOutcome::ContainerInspected {
            open_details: false,
            result: Ok(Box::new(inspect.clone())),
        });
        assert!(!app.dirty);

        let changed_inspect = ContainerInspectResponse {
            state: Some(bollard::secret::ContainerState {
                status: Some(ContainerStateStatusEnum::EXITED),
                ..Default::default()
            }),
            ..inspect
        };
        app.apply_docker_outcome(DockerOutcome::ContainerInspected {
            open_details: false,
            result: Ok(Box::new(changed_inspect)),
        });
        assert!(app.dirty);
    }

    #[tokio::test]
    async fn g_on_container_table_opens_logs_overlay_and_spawns_task() {
        let mock = MockDockerClient::default();
        let calls = mock.calls.clone();
        let mut app = App::with_client(mock);

        app.apply_docker_outcome(DockerOutcome::ContainersListed(Ok(vec![
            container_summary("a"),
        ])));
        app.container_table.select_row(0);

        let event = app
            .handle_key_event(KeyEvent::from(KeyCode::Char('g')))
            .unwrap();
        assert!(matches!(event, Some(AppEvent::GoToContainerLogs { .. })));

        app.events.send(event.unwrap());
        app.process_next_event().await.unwrap();

        assert!(matches!(app.overlay, Some(Overlay::Logs(_))));
        assert!(app.logs_task.is_some());

        // Let the spawned pump task run and reach the mock client.
        let _ = app.events.next().await.unwrap();
        assert!(
            calls
                .lock()
                .unwrap()
                .iter()
                .any(|c| c.starts_with("container_logs:"))
        );
    }

    #[tokio::test]
    async fn log_chunks_flow_through_the_channel_into_the_block() {
        let mock = MockDockerClient {
            log_lines: vec!["hello\n".to_string(), "world\n".to_string()],
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.open_logs("container-id".to_string(), "my-container".to_string());
        app.dirty = false;

        // First queued event is the coalesced chunk.
        app.process_next_event().await.unwrap();
        assert!(app.dirty);

        // Second is the clean end-of-stream marker.
        app.process_next_event().await.unwrap();

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.draw(f, f.area())).unwrap();
        let buffer: &Buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("hello"));
        assert!(content.contains("world"));
        assert!(content.contains("stream ended"));
    }

    #[tokio::test]
    async fn container_log_chunk_marks_dirty_for_the_current_session() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.open_logs("container-id".to_string(), "my-container".to_string());
        let session = app.logs_session;
        app.dirty = false;

        app.apply_docker_outcome(DockerOutcome::ContainerLogChunk {
            session,
            lines: vec!["hello".to_string()],
        });

        assert!(app.dirty);
    }

    #[tokio::test]
    async fn stale_session_chunk_is_ignored() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.open_logs("container-id".to_string(), "my-container".to_string());
        let stale_session = app.logs_session;
        // Re-open invalidates the previous session.
        app.open_logs("container-id".to_string(), "my-container".to_string());
        app.dirty = false;

        app.apply_docker_outcome(DockerOutcome::ContainerLogChunk {
            session: stale_session,
            lines: vec!["late".to_string()],
        });

        assert!(!app.dirty);
    }

    #[tokio::test]
    async fn back_aborts_the_log_stream_and_drops_late_chunks() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.open_logs("container-id".to_string(), "my-container".to_string());
        let session = app.logs_session;
        assert!(app.logs_task.is_some());

        app.events.send(AppEvent::Back);
        app.process_next_event().await.unwrap();

        assert!(app.overlay.is_none());
        assert!(app.logs_task.is_none());

        app.dirty = false;
        app.apply_docker_outcome(DockerOutcome::ContainerLogChunk {
            session,
            lines: vec!["late".to_string()],
        });
        assert!(!app.dirty);
    }

    #[tokio::test]
    async fn back_from_logs_opened_from_details_returns_to_details() {
        let mock = MockDockerClient {
            inspect: Some(ContainerInspectResponse {
                id: Some("container-id".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        // Open the details overlay first, then jump to logs from it.
        app.request_container_details_open("container-id".to_string());
        app.process_next_event().await.unwrap();
        assert!(matches!(app.overlay, Some(Overlay::Info(_))));

        app.open_logs("container-id".to_string(), "container-id".to_string());
        assert!(matches!(app.overlay, Some(Overlay::Logs(_))));

        app.events.send(AppEvent::Back);
        app.process_next_event().await.unwrap();

        // Back triggers a fresh inspect that re-opens the details overlay;
        // any stale log events still queued are dropped by the session guard.
        assert!(app.details_requested);
        while !matches!(app.overlay, Some(Overlay::Info(_))) {
            app.process_next_event().await.unwrap();
        }
    }

    #[tokio::test]
    async fn back_from_logs_opened_from_table_returns_to_table() {
        let mock = MockDockerClient::default();
        let calls = mock.calls.clone();
        let mut app = App::with_client(mock);

        app.open_logs("container-id".to_string(), "container-id".to_string());
        app.events.send(AppEvent::Back);
        app.process_next_event().await.unwrap();

        assert!(app.overlay.is_none());
        assert!(!app.details_requested);
        assert!(
            !calls
                .lock()
                .unwrap()
                .iter()
                .any(|c| c.starts_with("inspect_container"))
        );
    }

    #[tokio::test]
    async fn container_logs_ended_marks_the_block_clean_and_with_error() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.open_logs("container-id".to_string(), "my-container".to_string());
        let session = app.logs_session;
        app.dirty = false;

        app.apply_docker_outcome(DockerOutcome::ContainerLogsEnded {
            session,
            error: None,
        });
        assert!(app.dirty);

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.draw(f, f.area())).unwrap();
        let buffer: &Buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("stream ended"));

        app.dirty = false;
        app.apply_docker_outcome(DockerOutcome::ContainerLogsEnded {
            session,
            error: Some(DockerError::Other {
                message: "boom".to_string(),
            }),
        });
        assert!(app.dirty);
    }

    #[tokio::test]
    async fn char_l_does_not_switch_tabs_while_logs_open() {
        let mock = MockDockerClient::default();
        let mut app = App::with_client(mock);

        app.open_logs("container-id".to_string(), "my-container".to_string());
        let tab_before = app.selected_tab as usize;

        app.handle_key_event(KeyEvent::from(KeyCode::Char('l')))
            .unwrap();

        assert_eq!(app.selected_tab as usize, tab_before);
    }
}
