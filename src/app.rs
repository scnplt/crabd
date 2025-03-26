use crate::{docker::client::DockerClient, views::{container_info::{ContainerInfo, ContainerInfoData}, container_list_table::{ContainerData, ContainersTable}}};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use tokio::sync::mpsc::Receiver;
use std::{io, sync::{Arc, Mutex}, time::Duration};
use bollard::secret::{ContainerConfig, ContainerInspectResponse, ContainerState, ContainerStateStatusEnum, ContainerSummary, MountPoint, MountPointTypeEnum, Port, PortTypeEnum};
use ratatui::{text::Text, widgets::{Paragraph, Wrap}, DefaultTerminal};
 
pub enum CurrentScreen {
    List,
    Info,
}

enum NextOperation {
    None,
    Restart,
    Stop,
    Kill,
    Remove,
}

pub struct App {
    pub current_screen: CurrentScreen,
    pub should_exit: bool,
    pub show_all: bool,
    containers_table: ContainersTable,
    docker: DockerClient,
    next_operation: NextOperation,
    selected_container_id: String,
}

impl App {

    pub async fn new(client: DockerClient, containers: Arc<Mutex<Vec<ContainerSummary>>>) -> Self {
        let show_all = false;
        let containers_data: Vec<ContainerData> = map_to_container_data(containers.lock().unwrap().to_vec(),show_all);
        let mut first_container_id = "-1".to_string();

        if let Some(container) = containers_data.first() {
            first_container_id = container.id.clone()
        }

        Self {
            current_screen: CurrentScreen::List,
            should_exit: false,
            show_all,
            containers_table: ContainersTable::new(containers_data),
            docker: client,
            next_operation: NextOperation::None,
            selected_container_id: first_container_id
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal, rx: &mut Receiver<Vec<ContainerSummary>>) -> io::Result<()> {
        while !self.should_exit {
            match self.current_screen {
                CurrentScreen::List => self.draw_containers_table(terminal, rx).await,
                CurrentScreen::Info => self.draw_container_info(terminal).await
            }
            self.handle_events()?;
        }
        Ok(())
    }

    async fn draw_containers_table(&mut self, terminal: &mut DefaultTerminal, rx: &mut Receiver<Vec<ContainerSummary>>) {
        terminal.draw(|frame| self.containers_table.draw(frame)).unwrap();

        if let Ok(result) = rx.try_recv() {
            let updated_container_list: Vec<ContainerData> = map_to_container_data(result, self.show_all);
            self.containers_table.items = updated_container_list;
        }

        self.handle_container_operations().await;
    }

    async fn draw_container_info(&mut self, terminal: &mut DefaultTerminal) {
        self.update_selected_container_id();
        let data = self.docker.inspect_container(&self.selected_container_id).await;

        if let Ok(info) = data {
            let mut container_info = ContainerInfo::new(map_to_container_info_data(info));
            terminal.draw(|frame| container_info.render(frame, frame.area())).unwrap();
        }

        
        /* let info = if let Ok(info) = data {
               format!("{:?}", info)
        } else {
            "Lorem Ipsum".to_string()
        };


        let placeholder = Paragraph::new(Text::from(info)).left_aligned().wrap(Wrap { trim: true});
        terminal.draw(|frame| frame.render_widget(placeholder, frame.area())).unwrap(); */
    }

    async fn handle_container_operations(&mut self) {
        let result: Result<_, _> = match self.next_operation {
            NextOperation::Restart => self.docker.restart_container(&self.selected_container_id).await,
            NextOperation::Stop => self.docker.stop_container(&self.selected_container_id).await,
            NextOperation::Kill => self.docker.kill_container(&self.selected_container_id).await,
            NextOperation::Remove => self.docker.remove_container(&self.selected_container_id).await,
            _ => Err("Pass".into())
        };
        
        if result.is_ok() { self.next_operation = NextOperation::None; }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if crossterm::event::poll(Duration::from_millis(50))? {
            if let Event::Key(KeyEvent { code, ..}) = event::read()? {
                self.handle_key_event(code);
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.containers_table.next_row(),
            KeyCode::Char('k') | KeyCode::Up => self.containers_table.previous_row(),
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('t') => self.show_all = !self.show_all,
            KeyCode::Char('h') | KeyCode::Char('b') => self.go_to_list_screen(),
            KeyCode::Char('r') => self.restart_container(),
            KeyCode::Char('s') => self.stop_container(),
            KeyCode::Char('x') => self.kill_container(),
            KeyCode::Char('d') | KeyCode::Delete => self.remove_container(),
            KeyCode::Enter => self.go_to_info_screen(),
            _ => {}
        }
    }

    fn go_to_list_screen(&mut self) {
        if let CurrentScreen::Info { .. } = self.current_screen {
            self.current_screen = CurrentScreen::List;
        }
    }

    fn go_to_info_screen(&mut self) {
        if let CurrentScreen::List { .. } = self.current_screen {
            self.current_screen = CurrentScreen::Info;
        }
    }

    fn restart_container(&mut self) {
        self.update_selected_container_id();
        self.next_operation = NextOperation::Restart;
    }

    fn stop_container(&mut self) {
        self.update_selected_container_id();
        self.next_operation = NextOperation::Stop;
    }

    fn kill_container(&mut self) {
        self.update_selected_container_id();
        self.next_operation = NextOperation::Kill;
    }

    fn remove_container(&mut self) {
        self.update_selected_container_id();
        self.next_operation = NextOperation::Remove;
    }

    fn update_selected_container_id(&mut self) {
        let container_id = self.containers_table.get_current_container_id();
        self.selected_container_id = container_id;
    }
}

fn map_to_container_data(containers: Vec<ContainerSummary>, show_all: bool) -> Vec<ContainerData> {
    let mut result_list = containers.iter()
        .filter(|container| {
            let state = container.state.as_deref().unwrap_or("-").to_string();
            if show_all { true } else { String::eq(&state, "running") }
        })
        .map(|container| {
            let mut name = "NaN".to_string();
            if let Some(n) = container.names.as_deref().unwrap().first() {
                if let Some(stripped) = n.strip_prefix('/') {
                    name = stripped.to_string()
                }
            }
        
            ContainerData {
                id: container.id.as_deref().unwrap_or("-").to_string(),
                name,
                image: container.image.as_deref().unwrap_or("-").to_string(),
                state: container.state.as_deref().unwrap_or("-").to_string(),
                ports: container.ports.as_ref().map_or("-".to_string(), |p| get_ports_text(p)),
            }
        })
        .collect::<Vec<ContainerData>>();
    
    result_list.sort_by(|p, n| {
            let p_is_running = p.state.starts_with("r");
            let n_is_running = n.state.starts_with("r");

            match (p_is_running, n_is_running) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => p.state.cmp(&n.state),
            }
    });
    
    result_list
}

fn get_ports_text(ports: &[Port]) -> String {
    let mut filtered_ports: Vec<(u16, u16, PortTypeEnum)> = ports.iter()
        .filter(|p| p.public_port.is_some())
        .map(|p| (p.private_port, p.public_port.unwrap(), p.typ.unwrap()))
        .collect();

    filtered_ports.sort_by_key(|&(private, _, _)| private);
    filtered_ports.dedup();
    
    filtered_ports.iter()
        .map(|&(private, public, typ)| format!("{}:{}/{}", private, public, typ))
        .collect::<Vec<String>>()
        .join("\n")
}

fn map_to_container_info_data(container: ContainerInspectResponse) -> ContainerInfoData {
    let mut name = "NaN".to_string();
    if let Some(n) = container.name.as_deref() {
        if let Some(stripped) = n.strip_prefix('/') {
            name = stripped.to_string()
        }
    }

    let state = container.state.as_ref().unwrap_or(&ContainerState::default())
        .status.unwrap_or(ContainerStateStatusEnum::EMPTY).to_string();

    let mounts = container.mounts.as_ref().map_or("-".to_string(), |points| {
        let mut mp = points.iter()
            .map(|mp| {
                let source = match mp.typ {
                    Some(MountPointTypeEnum::VOLUME { .. }) => mp.name.clone().unwrap_or("-".to_string()),
                    Some(_) => mp.source.clone().unwrap_or("-".to_string()),
                    None => "-".to_string(),
                };

                let destination = mp.destination.clone().unwrap_or("-".to_string());
                format!("{} -> {}", source, destination)
            })
            .collect::<Vec<String>>();
        
        mp.sort_by_key(|v| v.clone());

        mp.join("\n")
    });

    let image = if let Some(config) = container.config {
        config.image.unwrap_or("-".to_string())
    } else {
        "-".to_string()
    };

    ContainerInfoData {
        id: container.id.as_deref().unwrap_or("-").to_string(),
        name,
        image,
        created: container.created.as_deref().unwrap_or("-").to_string(),
        state,
        mounts,
    }
}
