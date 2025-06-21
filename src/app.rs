use crate::components::container_info_block::{ContainerData, ContainerInfoBlock};
use crate::components::container_table::{ContainerTable, ContainerTableRow};
use crate::components::image_table::{ImageTable, ImageTableRow};
use crate::components::info_block::ScrollableInfoBlock;
use crate::components::network_table::{NetworkTable, NetworkTableRow};
use crate::components::volume_table::{VolumeTable, VolumeTableRow};
use crate::docker::client::DockerClient;
use crate::event::{AppEvent, Event, EventHandler};
use color_eyre::eyre::Result;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::palette::tailwind;
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;
use ratatui::widgets::Tabs;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event::Key, KeyCode, KeyEvent, KeyModifiers},
};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, FromRepr};

pub struct App {
    running: bool,
    events: EventHandler,
    docker_client: DockerClient,
    selected_tab: SelectedTab,
    container_table: ContainerTable,
    container_info: Option<Box<dyn ScrollableInfoBlock<Data = ContainerData>>>,
    volume_table: VolumeTable,
    network_table: NetworkTable,
    image_table: ImageTable,
}

impl App {
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

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.update_containers().await?;

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
            Event::Crossterm(event) => {
                if let Key(key_event) = event {
                    if let Some(event) = self.handle_key_event(key_event)? {
                        self.events.send(event);
                    }
                }
            }
            Event::App(app_event) => match app_event {
                AppEvent::Quit => self.quit(),
                AppEvent::UpdateContainers => self.update_containers().await?,
                AppEvent::UpdateContainerInfo(id) => self.update_container_details(id).await?,
                AppEvent::RestartContainer(id) => self.docker_client.restart_container(&id).await?,
                AppEvent::StopContainer(id) => self.docker_client.stop_container(&id).await?,
                AppEvent::KillContainer(id) => self.docker_client.kill_container(&id).await?,
                AppEvent::RemoveContainer(id) => self.remove_container(id).await?,
                AppEvent::GoToContainerDetails(id) => self.go_to_container_info(id).await?,
                AppEvent::UpdateVolumes => self.update_volumes().await?,
                AppEvent::RemoveVolume(name, force) => self.remove_volume(name, force).await?,
                AppEvent::UpdateNetworks => self.update_networks().await?,
                AppEvent::RemoveNetwork(name) => self.remove_network(name).await?,
                AppEvent::UpdateImages => self.update_images().await?,
                AppEvent::RemoveImage(id, force) => self.remove_image(id, force).await?,
                AppEvent::Back => self.container_info = None,
            },
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if key_event.code == KeyCode::Char('c') && key_event.modifiers == KeyModifiers::CONTROL {
            return Ok(Some(AppEvent::Quit))
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
                SelectedTab::Images => self.image_table.handle_key_event(key_event)?
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

    async fn go_to_container_info(&mut self, container_id: String) -> Result<()> {
        if let Some(data) = self.get_container_data(container_id).await {
            let mut container_info_block = ContainerInfoBlock::default();
            container_info_block.update_data(data);
            self.container_info = Some(Box::new(container_info_block));
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<Option<AppEvent>> {
        if let Some(info) = self.container_info.as_mut() {
            return info.tick();
        }

        let event = match self.selected_tab {
            SelectedTab::Containers => self.container_table.tick()?,
            SelectedTab::Volumes => self.volume_table.tick()?,
            SelectedTab::Networks => self.network_table.tick()?,
            SelectedTab::Images => self.image_table.tick()?
        };

        Ok(event)
    }

    fn quit(&mut self) {
        self.running = false;
    }

    async fn get_container_data(&self, container_id: String) -> Option<ContainerData> {
        if let Ok(data) = self.docker_client.inspect_container(&container_id).await {
            return Some(ContainerData::from(data));
        }
        None
    }

    async fn update_container_details(&mut self, container_id: String) -> Result<()> {
        if let Some(data) = self.get_container_data(container_id).await {
            if let Some(info_block) = self.container_info.as_mut() {
                info_block.update_data(data);
            }
        }
        Ok(())
    }

    async fn update_containers(&mut self) -> Result<()> {
        if let Ok(result) = self.docker_client.list_containers().await {
            let containers = ContainerTableRow::from_list(result);
            self.container_table.update_with_items(containers);
        }
        Ok(())
    }

    async fn update_volumes(&mut self) -> Result<()> {
        if let Some(result) = self.docker_client.list_volumes().await?.volumes {
            let volumes = VolumeTableRow::from_list(result);
            self.volume_table.update_with_items(volumes);
        }
        Ok(())
    }
    
    async fn update_networks(&mut self) -> Result<()> {
        if let Ok(result) = self.docker_client.list_networks().await {
            let networks = NetworkTableRow::from_list(result);
            self.network_table.update_with_items(networks);
        }
        Ok(())
    }

    async fn update_images(&mut self) -> Result<()> {
        if let Ok(result) = self.docker_client.list_images().await {
            let images = ImageTableRow::from_list(result);
            self.image_table.update_with_items(images);
        }
        Ok(())
    }

    async fn remove_container(&mut self, container_id: String) -> Result<()> {
        self.container_info = None;
        self.docker_client.remove_container(&container_id).await?;
        Ok(())
    }

    async fn remove_volume(&mut self, name: String, force: bool) -> Result<()> {
        if let Err(e) = self.docker_client.remove_volume(&name, force).await {
            self.volume_table.show_remove_volume_err(e.to_string());
        }
        Ok(())
    }

    async fn remove_network(&mut self, name: String) -> Result<()> {
        if let Err(e) = self.docker_client.remove_network(&name).await {
            self.network_table.show_remove_network_err(e.to_string());
        }
        Ok(())
    }

    async fn remove_image(&mut self, id: String, force: bool) -> Result<()> {
        if let Err(e) = self.docker_client.remove_image(&id, force).await {
            self.image_table.show_remove_image_err(e.to_string());
        }
        Ok(())
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
