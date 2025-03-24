use bollard::secret::ContainerSummary;
use ratatui::{
    buffer::Buffer, 
    layout::Rect, 
    style::Stylize, 
    symbols::border, 
    text::Line, 
    widgets::{Block, Widget}, 
    DefaultTerminal, 
    Frame
};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use tokio::{sync::{mpsc::Receiver}, task::futures};

use std::{io, sync::{Arc, Mutex}, time::Duration};

use crate::views::container_list::render_container_list;
 
pub enum CurrentScreen {
    List,
    Info,
}

pub struct App {
    pub current_screen: CurrentScreen,
    pub exit: bool,
    pub show_all: bool,
    pub containers: Vec<ContainerSummary>,
    selected_index: usize,
}

impl App {

    pub async fn new(containers: Arc<Mutex<Vec<ContainerSummary>>>) -> Self {
        let container_list = containers.lock().unwrap();

        Self {
            current_screen: CurrentScreen::List,
            exit: false,
            show_all: false,
            containers: container_list.to_vec(),
            selected_index: 0
        }
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal, rx: &mut Receiver<Vec<ContainerSummary>>) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            if let Ok(new_containers) = rx.try_recv() {
                self.containers = new_containers;
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
        
        render_container_list(frame, &self.containers, self.selected_index, self.show_all);
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
            KeyCode::Char('q') | KeyCode::Esc => self.exit = true,
            KeyCode::Char('t') => self.show_all = !self.show_all,
            KeyCode::Char('h') => {
                if let CurrentScreen::Info { .. } = self.current_screen {
                    self.current_screen = CurrentScreen::List;
                }
            }
            KeyCode::Enter => self.current_screen = CurrentScreen::Info,
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
}
