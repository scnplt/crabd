use crate::docker::client::{DockerApi, DockerClient};
use crate::docker::error::{DockerError, DockerResult};
use crate::event::{AppEvent, Event, EventHandler};
use crate::ui::container_info_block::{ContainerData, ContainerInfoBlock};
use crate::ui::container_table::{ContainerTable, ContainerTableRow};
use crate::ui::image_table::{ImageTable, ImageTableRow};
use crate::ui::info_block::ScrollableInfoBlock;
use crate::ui::network_table::{NetworkTable, NetworkTableRow};
use crate::ui::resource_table::{ResourceRow, ResourceTable};
use crate::ui::volume_table::{VolumeTable, VolumeTableRow};
use color_eyre::eyre::Result;
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
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, FromRepr};

pub struct App<C: DockerApi> {
    running: bool,
    events: EventHandler,
    docker_client: C,
    selected_tab: SelectedTab,
    container_table: ContainerTable,
    container_info: Option<Box<dyn ScrollableInfoBlock<Data = ContainerData>>>,
    volume_table: VolumeTable,
    network_table: NetworkTable,
    image_table: ImageTable,
}

impl App<DockerClient> {
    pub fn new() -> Result<Self> {
        Ok(Self {
            running: true,
            events: EventHandler::new(),
            docker_client: DockerClient::new()?,
            selected_tab: SelectedTab::default(),
            container_table: ContainerTable::default(),
            container_info: None,
            volume_table: VolumeTable::default(),
            network_table: NetworkTable::default(),
            image_table: ImageTable::default(),
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
            container_info: None,
            volume_table: VolumeTable::default(),
            network_table: NetworkTable::default(),
            image_table: ImageTable::default(),
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.update_containers().await;

        while self.running {
            terminal.draw(|frame| self.draw(frame, frame.area()))?;
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

        if let Some(info_block) = self.container_info.as_mut() {
            let _ = info_block.draw(frame, area);
        } else {
            render_title(frame, title_area);
            self.render_tabs(frame, tabs_area);
            let _ = self.render_selected_tab(frame, inner_area);
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
                if let Some(event) = self.handle_key_event(key_event)? {
                    self.events.send(event);
                }
            }
            Event::App(app_event) => match app_event {
                AppEvent::Quit => self.quit(),
                AppEvent::UpdateContainers => self.update_containers().await,
                AppEvent::UpdateContainerInfo(id) => self.update_container_details(id).await,
                AppEvent::RestartContainer(id) => self.restart_container(id).await,
                AppEvent::StopContainer(id) => self.stop_container(id).await,
                AppEvent::KillContainer(id) => self.kill_container(id).await,
                AppEvent::RemoveContainer(id) => self.remove_container(id).await,
                AppEvent::GoToContainerDetails(id) => self.go_to_container_info(id).await,
                AppEvent::UpdateVolumes => self.update_volumes().await,
                AppEvent::RemoveVolume(name, force) => self.remove_volume(name, force).await,
                AppEvent::UpdateNetworks => self.update_networks().await,
                AppEvent::RemoveNetwork(name) => self.remove_network(name).await,
                AppEvent::UpdateImages => self.update_images().await,
                AppEvent::RemoveImage(id, force) => self.remove_image(id, force).await,
                AppEvent::Back => self.container_info = None,
            },
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if key_event.code == KeyCode::Char('c') && key_event.modifiers == KeyModifiers::CONTROL {
            return Ok(Some(AppEvent::Quit));
        }

        if let Some(info) = self.container_info.as_mut() {
            return info.handle_key_event(key_event);
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

    async fn go_to_container_info(&mut self, container_id: String) {
        if let Some(data) = self.get_container_data(container_id).await {
            let mut container_info_block = ContainerInfoBlock::default();
            container_info_block.update_data(data);
            self.container_info = Some(Box::new(container_info_block));
        }
    }

    fn tick(&mut self) -> Result<Option<AppEvent>> {
        if let Some(info) = self.container_info.as_mut() {
            return info.tick();
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
        self.running = false;
    }

    fn report_err(&mut self, tab: SelectedTab, err: &DockerError) {
        match tab {
            SelectedTab::Containers => self.container_table.show_err(err),
            SelectedTab::Volumes => self.volume_table.show_err(err),
            SelectedTab::Networks => self.network_table.show_err(err),
            SelectedTab::Images => self.image_table.show_err(err),
        }
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

    async fn get_container_data(&mut self, container_id: String) -> Option<ContainerData> {
        let result = self.docker_client.inspect_container(&container_id).await;
        self.ok_or_report(SelectedTab::Containers, result)
            .map(ContainerData::from)
    }

    async fn update_container_details(&mut self, container_id: String) {
        let Some(data) = self.get_container_data(container_id).await else {
            return;
        };
        if let Some(info_block) = self.container_info.as_mut() {
            info_block.update_data(data);
        }
    }

    async fn restart_container(&mut self, container_id: String) {
        let result = self.docker_client.restart_container(&container_id).await;
        self.ok_or_report(SelectedTab::Containers, result);
    }

    async fn stop_container(&mut self, container_id: String) {
        let result = self.docker_client.stop_container(&container_id).await;
        self.ok_or_report(SelectedTab::Containers, result);
    }

    async fn kill_container(&mut self, container_id: String) {
        let result = self.docker_client.kill_container(&container_id).await;
        self.ok_or_report(SelectedTab::Containers, result);
    }

    async fn remove_container(&mut self, container_id: String) {
        let result = self.docker_client.remove_container(&container_id).await;
        self.ok_or_report(SelectedTab::Containers, result);
    }

    async fn update_containers(&mut self) {
        let result = self.docker_client.list_containers().await;
        if let Some(result) = self.ok_or_report(SelectedTab::Containers, result) {
            let containers = ContainerTableRow::from_list(result);
            self.container_table.update_with_items(containers);
        }
    }

    async fn update_volumes(&mut self) {
        let result = self.docker_client.list_volumes().await;
        if let Some(response) = self.ok_or_report(SelectedTab::Volumes, result)
            && let Some(volumes) = response.volumes
        {
            let volumes = VolumeTableRow::from_list(volumes);
            self.volume_table.update_with_items(volumes);
        }
    }

    async fn update_networks(&mut self) {
        let result = self.docker_client.list_networks().await;
        if let Some(result) = self.ok_or_report(SelectedTab::Networks, result) {
            let networks = NetworkTableRow::from_list(result);
            self.network_table.update_with_items(networks);
        }
    }

    async fn update_images(&mut self) {
        let result = self.docker_client.list_images().await;
        if let Some(result) = self.ok_or_report(SelectedTab::Images, result) {
            let images = ImageTableRow::from_list(result);
            self.image_table.update_with_items(images);
        }
    }

    async fn remove_volume(&mut self, name: String, force: bool) {
        let result = self.docker_client.remove_volume(&name, force).await;
        self.ok_or_report(SelectedTab::Volumes, result);
    }

    async fn remove_network(&mut self, name: String) {
        let result = self.docker_client.remove_network(&name).await;
        self.ok_or_report(SelectedTab::Networks, result);
    }

    async fn remove_image(&mut self, id: String, force: bool) {
        let result = self.docker_client.remove_image(&id, force).await;
        self.ok_or_report(SelectedTab::Images, result);
    }
}

fn render_title(frame: &mut Frame, area: Rect) {
    let title = " crabd".bold();
    frame.render_widget(title, area);
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
        ContainerInspectResponse, ContainerSummary, ImageSummary, Network, Volume,
        VolumeListResponse,
    };
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockDockerClient {
        containers: Vec<ContainerSummary>,
        volumes: Vec<Volume>,
        networks: Vec<Network>,
        images: Vec<ImageSummary>,
        inspect: Option<ContainerInspectResponse>,
        fail_with: Option<DockerError>,
        calls: Mutex<Vec<String>>,
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

    #[tokio::test]
    async fn update_containers_populates_table() {
        let mock = MockDockerClient {
            containers: vec![container_summary("a"), container_summary("b")],
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.update_containers().await;

        assert_eq!(app.container_table.table_info().items.len(), 2);
    }

    #[tokio::test]
    async fn update_volumes_populates_table() {
        let mock = MockDockerClient {
            volumes: vec![volume("a"), volume("b")],
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.update_volumes().await;

        assert_eq!(app.volume_table.table_info().items.len(), 2);
    }

    #[tokio::test]
    async fn update_networks_populates_table() {
        let mock = MockDockerClient {
            networks: vec![network("111111111111", "a"), network("222222222222", "b")],
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.update_networks().await;

        assert_eq!(app.network_table.table_info().items.len(), 2);
    }

    #[tokio::test]
    async fn update_images_populates_table() {
        let mock = MockDockerClient {
            images: vec![
                image("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd"),
                image("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abce"),
            ],
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.update_images().await;

        assert_eq!(app.image_table.table_info().items.len(), 2);
    }

    #[tokio::test]
    async fn restart_container_error_is_shown_in_footer() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.restart_container("container-id".to_string()).await;

        let err = app.container_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[tokio::test]
    async fn stop_container_error_is_shown_in_footer() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.stop_container("container-id".to_string()).await;

        let err = app.container_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[tokio::test]
    async fn kill_container_error_is_shown_in_footer() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.kill_container("container-id".to_string()).await;

        let err = app.container_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[tokio::test]
    async fn remove_volume_error_is_shown_in_footer() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::Conflict {
                message: "volume is in use".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.remove_volume("volume-name".to_string(), false).await;

        let err = app.volume_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: volume is in use");
    }

    #[tokio::test]
    async fn remove_network_error_is_shown_in_footer() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.remove_network("network-name".to_string()).await;

        let err = app.network_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[tokio::test]
    async fn remove_image_error_is_shown_in_footer() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::Conflict {
                message: "daemon says no".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.remove_image("image-id".to_string(), false).await;

        let err = app.image_table.table_info().err.clone().unwrap();
        assert_eq!(err, "[ERR] Conflict: daemon says no");
    }

    #[tokio::test]
    async fn list_failure_shows_error_on_owning_tab() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::DaemonUnreachable {
                details: "connection refused".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);
        assert_eq!(app.selected_tab as usize, SelectedTab::Containers as usize);

        app.update_images().await;

        assert!(app.image_table.table_info().err.is_some());
        assert!(app.container_table.table_info().err.is_none());
    }

    #[tokio::test]
    async fn list_failure_keeps_previous_items() {
        let mock = MockDockerClient {
            fail_with: Some(DockerError::DaemonUnreachable {
                details: "connection refused".to_string(),
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.update_containers().await;

        assert_eq!(app.container_table.table_info().items.len(), 0);
        assert!(app.container_table.table_info().err.is_some());
    }

    #[tokio::test]
    async fn go_to_container_info_opens_and_back_closes() {
        let mock = MockDockerClient {
            inspect: Some(ContainerInspectResponse {
                id: Some("container-id".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.go_to_container_info("container-id".to_string()).await;
        assert!(app.container_info.is_some());

        let event = app.handle_key_event(KeyEvent::from(KeyCode::Esc)).unwrap();
        assert!(matches!(event, Some(AppEvent::Back)));
    }

    #[tokio::test]
    async fn key_events_route_to_info_block_when_open() {
        let mock = MockDockerClient {
            inspect: Some(ContainerInspectResponse {
                id: Some("container-id".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut app = App::with_client(mock);

        app.go_to_container_info("container-id".to_string()).await;

        let tab_before = app.selected_tab as usize;
        app.handle_key_event(KeyEvent::from(KeyCode::Char('t')))
            .unwrap();
        assert_eq!(app.selected_tab as usize, tab_before);
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
}
