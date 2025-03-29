use crate::{
    docker::client::DockerClient, 
    views::{container_info::{ContainerInfo, ContainerInfoData}, 
    container_table::{ContainerData, ContainersTable}}
};

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use tokio::sync::mpsc::Receiver;
use std::{io, sync::{Arc, Mutex}, time::Duration};
use bollard::secret::ContainerSummary;
use ratatui::{
    buffer::Buffer, 
    layout::{Constraint, Layout, Rect}, 
    style::{palette::tailwind, Style}, 
    widgets::{Block, BorderType, Paragraph, Widget}, 
    DefaultTerminal
};
 
#[derive(PartialEq)]
pub enum CurrentScreen {
    List,
    Info,
}

#[derive(PartialEq)]
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
        let containers_data: Vec<ContainerData> = ContainerData::from_list(containers.lock().unwrap().to_vec(), show_all);
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
            selected_container_id: first_container_id,
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal, rx: &mut Receiver<Vec<ContainerSummary>>) -> io::Result<()> {
        while !self.should_exit {
            let vertical_layout = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]);
            let [content_area, footer_area] = vertical_layout.areas(terminal.get_frame().area());

            match self.current_screen {
                CurrentScreen::List => self.draw_containers_table(content_area, terminal, rx),
                CurrentScreen::Info => self.draw_container_info(content_area, terminal).await
            }
            
            render_footer(footer_area, terminal.get_frame().buffer_mut(), &self.current_screen, self.show_all);

            self.handle_events()?;
            self.handle_container_operations().await;
        }
        Ok(())
    }

    fn draw_containers_table(&mut self, area: Rect, terminal: &mut DefaultTerminal, rx: &mut Receiver<Vec<ContainerSummary>>) {
        terminal.draw(|frame| self.containers_table.draw(frame, area)).unwrap();
        if let Ok(result) = rx.try_recv() {
            self.containers_table.items = ContainerData::from_list(result, self.show_all);
        }
    }
    
    async fn draw_container_info(&mut self, area: Rect, terminal: &mut DefaultTerminal) {
        self.update_selected_container_id();
        let data = self.docker.inspect_container(&self.selected_container_id).await;
        if let Ok(info) = data {
            self.container_info.data = ContainerInfoData::from(info);
            terminal.draw(|frame| self.container_info.draw(frame, area)).unwrap();
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
        if self.next_operation != NextOperation::None { return; }

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
        if self.current_screen == CurrentScreen::List {
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

fn render_footer(area: Rect, buf: &mut Buffer, current_screen: &CurrentScreen, show_all: bool) {
    let border_style = Style::new().fg(tailwind::BLUE.c400);
    let footer_style = Style::new().fg(tailwind::SLATE.c200);

    let title = match current_screen {
        CurrentScreen::Info => " <Esc/Q> back | <R> restart | <S> stop | <X> kill | <Del/D> remove".to_string(),
        CurrentScreen::List => {
            let toggle_text = if show_all { "All" } else { "Running" };
            format!(" <Ent> details | <T> {} | <R> restart | <S> stop | <X> kill | <Del/D> remove", toggle_text)
        }
    };

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(border_style);

    Paragraph::new(title)
        .style(footer_style)
        .left_aligned()
        .block(block)
        .render(area, buf);
}
