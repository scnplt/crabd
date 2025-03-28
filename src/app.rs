use crate::{
    docker::client::DockerClient, 
    views::{container_info::{ContainerInfo, ContainerInfoData}, 
    container_list_table::{ContainerData, ContainersTable}}
};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use tokio::sync::mpsc::Receiver;
use std::{io, sync::{Arc, Mutex}, time::Duration};
use bollard::secret::{ContainerInspectResponse, ContainerState, ContainerStateStatusEnum, ContainerSummary, MountPointTypeEnum, Port, PortBinding, PortTypeEnum};
use ratatui::DefaultTerminal;
 
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
    container_info: ContainerInfo,
    docker: DockerClient,
    next_operation: NextOperation,
    selected_container_id: String,
}

impl App {

    pub async fn new(client: DockerClient, containers: Arc<Mutex<Vec<ContainerSummary>>>) -> Self {
        let show_all = true;
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
            container_info: ContainerInfo::default(),
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
            self.handle_container_operations().await;
        }
        Ok(())
    }

    async fn draw_containers_table(&mut self, terminal: &mut DefaultTerminal, rx: &mut Receiver<Vec<ContainerSummary>>) {
        terminal.draw(|frame| self.containers_table.draw(frame)).unwrap();

        if let Ok(result) = rx.try_recv() {
            let updated_container_list: Vec<ContainerData> = map_to_container_data(result, self.show_all);
            self.containers_table.items = updated_container_list;
        }
    }

    async fn draw_container_info(&mut self, terminal: &mut DefaultTerminal) {
        self.update_selected_container_id();
        let data = self.docker.inspect_container(&self.selected_container_id).await;

        if let Ok(info) = data {
            self.container_info.data = map_to_container_info_data(info);
            terminal.draw(|frame| self.container_info.draw(frame)).unwrap();
        }
    }

    async fn handle_container_operations(&mut self) {
        let result: Result<_, _> = match self.next_operation {
            NextOperation::Restart => self.docker.restart_container(&self.selected_container_id).await,
            NextOperation::Stop => self.docker.stop_container(&self.selected_container_id).await,
            NextOperation::Kill => self.docker.kill_container(&self.selected_container_id).await,
            NextOperation::Remove => {
                self.current_screen = CurrentScreen::List;
                self.docker.remove_container(&self.selected_container_id).await
            },
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
        match self.current_screen {
            CurrentScreen::Info => self.container_info.handle_key_event(code),
            CurrentScreen::List => self.containers_table.handle_key_event(code),
        }
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.back(),
            KeyCode::Enter => self.go_to_info_screen(),
            KeyCode::Char('t') => self.show_all = !self.show_all,
            KeyCode::Char('r') => self.restart_container(),
            KeyCode::Char('s') => self.stop_container(),
            KeyCode::Char('x') => self.kill_container(),
            KeyCode::Delete | KeyCode::Char('d') => self.remove_container(),
            _ => {}
        }
    }

    fn back(&mut self) {
        match self.current_screen {
            CurrentScreen::Info => self.current_screen = CurrentScreen::List,
            CurrentScreen::List => self.should_exit = true
        }
    }

    fn go_to_info_screen(&mut self) {
        if let CurrentScreen::List = self.current_screen { 
            self.current_screen = CurrentScreen::Info 
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
            let name: String = container.names.as_deref()
                .and_then(|names| names.first())
                .and_then(|name| name.strip_prefix("/"))
                .map_or("NaN".to_string(), |name| name.to_string());
        
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
    .map(|ports| {
        ports.iter().map(|(port, bindings)| {
            let (ipv4_binding, ipv6_binding) = bindings.as_ref().map(|b| {
                let ipv4 = b.iter().find(|pb| pb.host_ip == Some("0.0.0.0".to_string()));
                let ipv6 = b.iter().find(|pb| pb.host_ip == Some("::".to_string()));
                (ipv4, ipv6)
            }).unwrap_or((None, None));

            let port_number = port.split('/').next().unwrap_or("");
            let protocol = port.split('/').nth(1).unwrap_or("");

            let ip_map = |pb: &PortBinding| -> String { format!("{}:{}", pb.host_ip.as_deref().unwrap_or(""), pb.host_port.as_deref().unwrap_or("")) };
            let ipv4_str = ipv4_binding.map(ip_map).unwrap_or_default();
            let ipv6_str = ipv6_binding.map(ip_map).unwrap_or_default();

            match (ipv4_str.is_empty(), ipv6_str.is_empty()) {
                (false, false) => format!("{} | {} -> {}/{}", ipv4_str, ipv6_str, port_number, protocol),
                (false, true) => format!("{} -> {}/{}", ipv4_str, port_number, protocol),
                (true, false) => format!("{} -> {}/{}", ipv6_str, port_number, protocol),
                _ => "".to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
    })
    .unwrap_or_else(|| "-".to_string());

    let default_state = ContainerState::default();
    let state_info = container.state.as_ref().unwrap_or(&default_state);
    let state: String = state_info.status.unwrap_or(ContainerStateStatusEnum::EMPTY).to_string();
    let start_time: String = state_info.started_at.as_deref().unwrap_or("-").to_string();

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

    ContainerInfoData {
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
