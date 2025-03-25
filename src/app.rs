use crate::views::container_list_table::{ContainersTable, ContainerData};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use tokio::sync::mpsc::Receiver;
use std::{io, sync::{Arc, Mutex}, time::Duration};
use bollard::secret::{ContainerSummary, Port, PortTypeEnum};
use ratatui::{
    style::Stylize, 
    symbols::border, 
    text::Line, 
    widgets::Block, 
    DefaultTerminal, 
    Frame
};
 
pub enum CurrentScreen {
    List,
    Info,
}

pub struct App {
    pub current_screen: CurrentScreen,
    pub should_exit: bool,
    pub show_all: bool,
    containers_table: ContainersTable,
}

impl App {

    pub async fn new(containers: Arc<Mutex<Vec<ContainerSummary>>>) -> Self {
        let show_all = false;
        let containers_data: Vec<ContainerData> = map_to_container_data(containers.lock().unwrap().to_vec(),show_all);

        Self {
            current_screen: CurrentScreen::List,
            should_exit: false,
            show_all,
            containers_table: ContainersTable::new(containers_data)
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal, rx: &mut Receiver<Vec<ContainerSummary>>) -> io::Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| self.containers_table.draw(frame))?;

            if let Ok(result) = rx.try_recv() {
                let updated_container_list: Vec<ContainerData> = map_to_container_data(result, self.show_all);
                self.containers_table.items = updated_container_list;
            }

            self.handle_events()?;
        }

        Ok(())
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
            KeyCode::Char('h') => self.go_to_list_screen(),
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
}

fn get_filtered_containers(containers: Vec<ContainerData>, show_all: bool) -> Vec<ContainerData> {
    if show_all { return containers; }

    containers.into_iter()
        .filter(|container| !String::eq(&container.state, &"exited".to_string()))
        .collect()
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
                ports: container.ports.as_ref().map_or("-".to_string(), get_ports_text),
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

fn get_ports_text(ports: &Vec<Port>) -> String {
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