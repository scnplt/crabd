use crate::views::container_list::render_container_list;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use tokio::sync::mpsc::Receiver;
use std::{io, sync::{Arc, Mutex}, time::Duration};
use bollard::secret::ContainerSummary;
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
    pub containers: Vec<ContainerSummary>,
    selected_index: usize,
}

impl App {

    pub async fn new(containers: Arc<Mutex<Vec<ContainerSummary>>>) -> Self {
        let show_all = false;
        let containers_list = containers.lock().unwrap().to_vec();

        Self {
            current_screen: CurrentScreen::List,
            should_exit: false,
            show_all,
            containers: get_filtered_containers(containers_list, show_all),
            selected_index: 0
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal, rx: &mut Receiver<Vec<ContainerSummary>>) -> io::Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| self.draw(frame))?;

            if let Ok(result) = rx.try_recv() {
                let updated_container_list = get_filtered_containers(result, self.show_all);
                let new_last_index = updated_container_list.len() - 1;
                
                if self.selected_index > new_last_index {
                    self.selected_index = new_last_index;
                }

                self.containers = updated_container_list;
            }
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        let title = Line::from(self.get_title().bold());
        let block = Block::bordered()
            .title(title.left_aligned())
            .title_bottom(self.get_first_bottom_line().left_aligned())
            .title_bottom(self.get_second_bottom_line().right_aligned())
            .border_set(border::THICK);
        
        render_container_list(frame, &self.containers, self.selected_index);
        frame.render_widget(block, frame.area());
    }

    fn get_title(&self) -> &str {
        match self.current_screen {
            CurrentScreen::List => "Containers ",
            CurrentScreen::Info => "Details ",
        }
    }

    fn get_first_bottom_line(&self) -> Line<'_> {
        if let CurrentScreen::Info { .. } = self.current_screen {
            return Line::from(vec![
                " Back: ".into(),
                "<H> ".blue().bold(),
            ]);
        }

        let mut values = vec![
            " Details: ".into(),
            "<Ent>".blue().bold(),
            " Show All: ".into(),
            "<T> ".blue().bold(),
        ];

        if self.show_all {
            values[2] = " Running: ".into();
        }

        Line::from(values)
    }

    fn get_second_bottom_line(&self) -> Line<'_> {
        let values = vec![
            " Restart: ".into(),
            "<R>".green().bold(),
            " Stop: ".into(),
            "<S>".red().bold(),
            " Kill: ".into(),
            "<X> ".red().bold(),
        ];

        Line::from(values)
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
            KeyCode::Char('j') | KeyCode::Down => self.select_next_container(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous_container(),
            KeyCode::Char('q') | KeyCode::Esc => self.should_exit = true,
            KeyCode::Char('t') => self.show_all = !self.show_all,
            KeyCode::Char('h') => self.go_to_list_screen(),
            KeyCode::Enter => self.go_to_info_screen(),
            _ => {}
        }
    }

    fn select_previous_container(&mut self) {
        if self.selected_index == 0 {
            self.selected_index = self.containers.len() - 1;
            return;
        }
        self.selected_index -= 1;
    }

    fn select_next_container(&mut self) {
        if self.selected_index == self.containers.len() - 1 {
            self.selected_index = 0;
            return;
        }
        self.selected_index += 1;
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

fn get_filtered_containers(containers: Vec<ContainerSummary>, show_all: bool) -> Vec<ContainerSummary> {
    if show_all { return containers; }

    containers.iter()
        .filter(|container| container.state.as_deref() != Some("exited"))
        .cloned()
        .collect()
}
