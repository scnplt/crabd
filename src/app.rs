use crate::components::component::Component;
use crate::components::container_info::{ContainerData, ContainerInfoBlock};
use crate::components::container_table::{ContainerTable, ContainerTableRow};
use crate::docker::client::DockerClient;
use crate::event::{AppEvent, Event, EventHandler};
use color_eyre::eyre::Result;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{Event::Key, KeyCode, KeyEvent, KeyModifiers},
};

#[derive(PartialEq)]
pub enum Screen {
    ContainerList,
    ContainerInfo,
}

pub struct App {
    running: bool,
    events: EventHandler,
    components: Vec<Box<dyn Component>>,
    docker_client: DockerClient,
    current_screen: Screen,
}

impl App {
    pub fn new() -> Result<Self> {
        Ok(Self {
            running: true,
            events: EventHandler::new(),
            components: vec![Box::new(ContainerTable::default())],
            docker_client: DockerClient::new()?,
            current_screen: Screen::ContainerList,
        })
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while self.running {
            terminal.draw(|frame| {
                self.components.iter_mut()
                    .filter(|c| c.is_showing(&self.current_screen))
                    .for_each(|c| c.draw(frame, frame.area()).unwrap());
            })?;

            self.process_next_event().await?;
        }
        Ok(())
    }

    // Since different screens will be added,
    // key handling for container operations is done in the component, not in the application.
    async fn process_next_event(&mut self) -> Result<()> {
        match self.events.next().await? {
            Event::Tick => self.tick(),
            Event::Crossterm(event) => {
                if let Key(key_event) = event { 
                    self.handle_key_events(key_event)?
                }
            },
            Event::App(app_event) => match app_event {
                AppEvent::Quit => self.quit(),
                AppEvent::UpdateContainers => self.update_containers().await?,
                AppEvent::UpdateContainerInfo(id) => self.update_container_details(id).await?,
                AppEvent::RestartContainer(id) => self.docker_client.restart_container(&id).await?,
                AppEvent::StopContainer(id) => self.docker_client.stop_container(&id).await?,
                AppEvent::KillContainer(id) => self.docker_client.kill_container(&id).await?,
                AppEvent::RemoveContainer(id) => {
                    self.go_back_from_container_details();
                    self.docker_client.remove_container(&id).await?
                },
                AppEvent::GoToDetails(id) => self.go_to_container_info(id).await?,
                AppEvent::Back => self.go_back_from_container_details(),
            },
        }
        Ok(())
    }

    fn go_back_from_container_details(&mut self) {
        if self.current_screen == Screen::ContainerInfo {
            self.components.pop();
            self.current_screen = Screen::ContainerList
        }
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<()> {
        match key_event.code {
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            _ => {
                self.components.iter_mut()
                    .filter(|c| c.is_showing(&self.current_screen))
                    .for_each(|c| {
                        if let Some(event) = c.handle_key_event(key_event).unwrap() {
                            self.events.send(event);
                        }
                    });
            },
        }
        Ok(())
    }

    async fn go_to_container_info(&mut self, container_id: String) -> Result<()> {
        if let Some(data) = self.get_container_data(container_id).await {
            let mut container_info_block = ContainerInfoBlock::default();
            container_info_block.update_data(data);
            self.components.push(Box::new(container_info_block));
            self.current_screen = Screen::ContainerInfo;
        }
        Ok(())
    }

    fn tick(&mut self) {
        self.components.iter_mut().for_each(|c| {
            if let Some(event) = c.tick().unwrap() {
                self.events.send(event);
            }
        });
    }

    fn quit(&mut self) {
        self.running = false;
    }

    async fn get_container_data(&self, container_id: String) -> Option<ContainerData> {
        if let Ok(data) = self.docker_client.inspect_container(&container_id).await {
            return Some(ContainerData::from(data))
        }
        None
    }

    async fn update_container_details(&mut self, container_id: String) -> Result<()> {
        if self.current_screen != Screen::ContainerInfo { return Ok(()) }

        if let Some(data) = self.get_container_data(container_id).await {
            self.components.iter_mut()
                .find_map(|c| c.as_any_mut().downcast_mut::<ContainerInfoBlock>()).unwrap()
                .update_data(data);
        }

        Ok(())
    }

    async fn update_containers(&mut self) -> Result<()> {
        if self.current_screen != Screen::ContainerList { return Ok(()) }

        if let Ok(result) = self.docker_client.list_containers().await {
            let containers = ContainerTableRow::from_list(result);
            self.components.iter_mut()
                .find_map(|c| c.as_any_mut().downcast_mut::<ContainerTable>()).unwrap()
                .update_with_items(containers);
        }

        Ok(()) 
    }
}
