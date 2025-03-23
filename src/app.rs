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

use std::io;
 
 #[derive(Default)]
pub enum CurrentScreen {
    #[default]
    List,
    Info,
}

#[derive(Default)]
pub enum BarState {
    #[default]
    Default,
    ShowAll,
}

#[derive(Default)]
pub struct App {
    pub current_screen: CurrentScreen,
    pub bar_state: BarState,
    pub exit: bool,
}

impl App {

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn get_title(&self) -> &str {
        match self.current_screen {
            CurrentScreen::List => " Containers ",
            CurrentScreen::Info => " Details ",
        }
    }

    fn get_first_bottom_line(&self) -> Line<'_> {
        if let CurrentScreen::Info { .. } = self.current_screen {
            return Line::from("");
        }

        let mut values = vec![
            " Down: ".into(),
            "<J>".blue().bold(),
            " Up: ".into(),
            "<K>".blue().bold(),
            " Details: ".into(),
            "<Ent>".blue().bold(),
            " Show All: ".into(),
            "<T>".blue().bold(),
            " Quit: ".into(),
            "<Q> ".blue().bold(),
        ];

        if let BarState::ShowAll { .. } = self.bar_state {
            values[6] = " Running: ".into();
            values[7] = "<T>".blue().bold(); 
        }

        Line::from(values)
    }

    fn get_second_bottom_line(&self) -> Line<'_> {
        let mut values = vec![
            " Restart: ".into(),
            "<R>".green().bold(),
            " Stop: ".into(),
            "<S>".red().bold(),
            " Kill: ".into(),
            "<X> ".red().bold(),
            "".into(),
            "".into(),
        ];

        if let CurrentScreen::Info { .. } = self.current_screen {
            values[6] = "Back: ".into();
            values[7] = "<H> ".blue().bold()
        }

        Line::from(values)
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit = true,
            KeyCode::Char('t') => {
                self.bar_state = match self.bar_state {
                    BarState::Default => BarState::ShowAll,
                    _ => BarState::Default
                };
            },
            KeyCode::Char('h') => {
                if let CurrentScreen::Info { .. } = self.current_screen {
                    self.current_screen = CurrentScreen::List;
                }
            }
            KeyCode::Enter => self.current_screen = CurrentScreen::Info,
            _ => {}
        }
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(self.get_title().bold());
        let block = Block::bordered()
            .title(title.left_aligned())
            .title_bottom(self.get_first_bottom_line().left_aligned())
            .title_bottom(self.get_second_bottom_line().right_aligned())
            .border_set(border::THICK);
        
        block.render(area, buf);

    }
}
