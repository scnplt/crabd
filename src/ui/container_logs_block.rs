use std::collections::VecDeque;

use crate::docker::error::DockerError;
use crate::event::AppEvent;

use super::common::{render_footer, render_scrollbar};
use super::info_block::ScrollInfo;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize, palette::tailwind},
    text::Line,
    widgets::{Block, BorderType, Paragraph},
};

/// Caps the number of buffered log lines so a chatty container can't grow
/// memory usage without bound.
pub const MAX_LOG_LINES: usize = 2000;

/// Full-screen container log viewer. Unlike `ScrollableInfoBlock`
/// implementations, data arrives incrementally (append, not replace) and
/// `End`/`Home` control follow mode rather than horizontal scroll extremes,
/// so this is a standalone widget rather than a `ScrollableInfoBlock`.
pub struct ContainerLogsBlock {
    // Kept for API completeness (mirrors the id/name pair `App` passes in);
    // not currently read back since `App` tracks the id separately for the pump task.
    #[allow(dead_code)]
    id: String,
    name: String,
    lines: VecDeque<String>,
    /// When true, the view stays pinned to the newest line as it arrives.
    follow: bool,
    scroll_info: ScrollInfo,
    max_line_width: usize,
    /// `Some(None)` = the stream ended cleanly; `Some(Some(err))` = it ended with an error.
    ended: Option<Option<DockerError>>,
}

impl ContainerLogsBlock {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            lines: VecDeque::new(),
            follow: true,
            scroll_info: ScrollInfo::default(),
            max_line_width: 0,
            ended: None,
        }
    }

    #[allow(dead_code)]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Appends new lines, enforcing `MAX_LOG_LINES`. Returns true if anything changed.
    pub fn push_lines(&mut self, lines: Vec<String>) -> bool {
        if lines.is_empty() {
            return false;
        }

        for line in lines {
            self.max_line_width = self.max_line_width.max(line.len());
            self.lines.push_back(line);
        }

        while self.lines.len() > MAX_LOG_LINES {
            self.lines.pop_front();
            // Not following: the trimmed line was above the viewport, so shift
            // the viewport up with it to avoid a visual jump.
            if !self.follow {
                self.scroll_info.vertical = self.scroll_info.vertical.saturating_sub(1);
            }
        }

        true
    }

    /// Marks the stream as finished, with an optional error. Always dirties the view.
    pub fn mark_ended(&mut self, error: Option<DockerError>) -> bool {
        self.ended = Some(error);
        true
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        let event = match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => Some(AppEvent::Back),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::PageUp | KeyCode::Home => {
                self.scroll_up();
                self.follow = false;
                None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::PageDown => {
                self.scroll_down();
                if self.scroll_info.vertical == self.scroll_info.max_vertical {
                    self.follow = true;
                }
                None
            }
            KeyCode::End | KeyCode::Char('f') => {
                self.follow = true;
                None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.scroll_left();
                None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.scroll_right();
                None
            }
            _ => None,
        };
        Ok(event)
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        use Constraint::{Length, Min};

        let vertical_layout = Layout::vertical([Min(0), Length(1), Length(3)]);
        let [content_area, horizontal_scrollbar_area, footer_area] = vertical_layout.areas(area);

        let horizontal_layout = Layout::horizontal([Min(0), Length(1)]);
        let [info_area, vertical_scrollbar_area] = horizontal_layout.areas(content_area);

        let content_lines = self.content_lines();

        self.scroll_info.max_horizontal = self
            .max_line_width
            .saturating_sub(info_area.width as usize - 2);
        self.scroll_info.max_vertical = content_lines
            .len()
            .saturating_sub(info_area.height as usize - 2);

        if self.follow {
            self.scroll_info.vertical = self.scroll_info.max_vertical;
        }

        self.render_content(frame, info_area, content_lines);

        self.scroll_info.vertical_state = self
            .scroll_info
            .vertical_state
            .content_length(self.scroll_info.max_vertical)
            .position(self.scroll_info.vertical);

        render_scrollbar(
            frame,
            vertical_scrollbar_area,
            &mut self.scroll_info.vertical_state,
            true,
        );

        self.scroll_info.horizontal_state = self
            .scroll_info
            .horizontal_state
            .content_length(self.scroll_info.max_horizontal)
            .position(self.scroll_info.horizontal);

        render_scrollbar(
            frame,
            horizontal_scrollbar_area,
            &mut self.scroll_info.horizontal_state,
            false,
        );

        render_footer(
            frame,
            footer_area,
            get_footer_text(self.follow, self.ended.is_some()),
            None,
        );

        Ok(())
    }

    fn scroll_up(&mut self) {
        if self.scroll_info.vertical != 0 {
            self.scroll_info.vertical -= 1;
        }
    }

    fn scroll_down(&mut self) {
        if self.scroll_info.vertical != self.scroll_info.max_vertical {
            self.scroll_info.vertical += 1;
        }
    }

    fn scroll_left(&mut self) {
        if self.scroll_info.horizontal != 0 {
            self.scroll_info.horizontal -= 1;
        }
    }

    fn scroll_right(&mut self) {
        if self.scroll_info.horizontal != self.scroll_info.max_horizontal {
            self.scroll_info.horizontal += 1;
        }
    }

    fn content_lines(&self) -> Vec<Line<'static>> {
        if self.lines.is_empty() && self.ended.is_none() {
            return vec![Line::from("Waiting for logs...").dim()];
        }

        let mut lines: Vec<Line<'static>> = self
            .lines
            .iter()
            .cloned()
            .map(Line::from)
            .collect::<Vec<_>>();

        if let Some(error) = &self.ended {
            lines.push(Line::from("-- log stream ended --").dim());
            if let Some(error) = error {
                lines.push(Line::from(error.to_string()).dim());
            }
        }

        lines
    }

    fn render_content(&self, frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
        let block_style = Style::new().fg(tailwind::BLUE.c400);

        let title = Line::from(format!("Logs: {}", self.name)).fg(tailwind::SLATE.c200);

        let block = Block::bordered()
            .border_type(BorderType::Plain)
            .border_style(block_style)
            .title(title);

        let paragraph = Paragraph::new(lines)
            .block(block)
            .scroll((
                self.scroll_info.vertical as u16,
                self.scroll_info.horizontal as u16,
            ))
            .left_aligned();

        frame.render_widget(paragraph, area);
    }
}

fn get_footer_text(follow: bool, ended: bool) -> String {
    let follow_state = if follow { "on" } else { "off" };
    let mut text = format!(" <Esc/Q> back | <F/End> follow [{follow_state}] | <J/K> scroll");
    if ended {
        text.push_str(" | stream ended");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    fn block() -> ContainerLogsBlock {
        ContainerLogsBlock::new("container-id".to_string(), "my-container".to_string())
    }

    #[test]
    fn new_starts_following_with_no_lines() {
        let block = block();
        assert!(block.follow);
        assert!(block.lines.is_empty());
        assert!(block.ended.is_none());
    }

    #[test]
    fn push_lines_reports_change_and_updates_max_width() {
        let mut block = block();

        let changed = block.push_lines(vec!["short".to_string(), "a longer line".to_string()]);
        assert!(changed);
        assert_eq!(block.lines.len(), 2);
        assert_eq!(block.max_line_width, "a longer line".len());

        let unchanged = block.push_lines(vec![]);
        assert!(!unchanged);
    }

    #[test]
    fn push_lines_enforces_cap_and_adjusts_scroll_when_not_following() {
        let mut block = block();
        block.follow = false;
        block.scroll_info.vertical = 5;

        let lines = (0..MAX_LOG_LINES + 10)
            .map(|i| format!("line {i}"))
            .collect();
        block.push_lines(lines);

        assert_eq!(block.lines.len(), MAX_LOG_LINES);
        // 10 lines were trimmed from the front while not following.
        assert_eq!(block.scroll_info.vertical, 0);
    }

    #[test]
    fn push_lines_does_not_adjust_scroll_while_following() {
        let mut block = block();
        block.scroll_info.vertical = 5;

        let lines = (0..MAX_LOG_LINES + 10)
            .map(|i| format!("line {i}"))
            .collect();
        block.push_lines(lines);

        assert_eq!(block.lines.len(), MAX_LOG_LINES);
        assert_eq!(block.scroll_info.vertical, 5);
    }

    #[test]
    fn mark_ended_sets_state_and_reports_change() {
        let mut block = block();
        assert!(block.mark_ended(None));
        assert!(matches!(block.ended, Some(None)));

        assert!(block.mark_ended(Some(DockerError::Other {
            message: "boom".to_string(),
        })));
        assert!(matches!(block.ended, Some(Some(_))));
    }

    #[test]
    fn esc_and_q_yield_back() {
        let mut block = block();

        let event = block
            .handle_key_event(KeyEvent::from(KeyCode::Esc))
            .unwrap();
        assert!(matches!(event, Some(AppEvent::Back)));

        let event = block
            .handle_key_event(KeyEvent::from(KeyCode::Char('q')))
            .unwrap();
        assert!(matches!(event, Some(AppEvent::Back)));
    }

    #[test]
    fn scrolling_up_disengages_follow() {
        let mut block = block();
        block.scroll_info.max_vertical = 10;
        block.scroll_info.vertical = 5;
        assert!(block.follow);

        block.handle_key_event(KeyEvent::from(KeyCode::Up)).unwrap();

        assert!(!block.follow);
        assert_eq!(block.scroll_info.vertical, 4);
    }

    #[test]
    fn end_and_f_reengage_follow() {
        let mut block = block();
        block.follow = false;

        block
            .handle_key_event(KeyEvent::from(KeyCode::End))
            .unwrap();
        assert!(block.follow);

        block.follow = false;
        block
            .handle_key_event(KeyEvent::from(KeyCode::Char('f')))
            .unwrap();
        assert!(block.follow);
    }

    #[test]
    fn scrolling_down_to_the_bottom_reengages_follow() {
        let mut block = block();
        block.follow = false;
        block.scroll_info.max_vertical = 5;
        block.scroll_info.vertical = 4;

        block
            .handle_key_event(KeyEvent::from(KeyCode::Down))
            .unwrap();

        assert_eq!(block.scroll_info.vertical, 5);
        assert!(block.follow);
    }

    #[test]
    fn horizontal_scroll_is_clamped() {
        let mut block = block();
        block.scroll_info.max_horizontal = 1;

        block
            .handle_key_event(KeyEvent::from(KeyCode::Right))
            .unwrap();
        block
            .handle_key_event(KeyEvent::from(KeyCode::Right))
            .unwrap();
        assert_eq!(block.scroll_info.horizontal, 1);

        block
            .handle_key_event(KeyEvent::from(KeyCode::Left))
            .unwrap();
        block
            .handle_key_event(KeyEvent::from(KeyCode::Left))
            .unwrap();
        assert_eq!(block.scroll_info.horizontal, 0);
    }

    #[test]
    fn get_footer_text_variants() {
        assert_eq!(
            get_footer_text(true, false),
            " <Esc/Q> back | <F/End> follow [on] | <J/K> scroll"
        );
        assert_eq!(
            get_footer_text(false, false),
            " <Esc/Q> back | <F/End> follow [off] | <J/K> scroll"
        );
        assert!(get_footer_text(true, true).ends_with("| stream ended"));
    }

    #[test]
    fn draw_pins_to_bottom_while_following() {
        let mut block = block();
        let lines = (0..50).map(|i| format!("line {i}")).collect();
        block.push_lines(lines);

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| block.draw(f, f.area()).unwrap()).unwrap();

        let buffer: &Buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        // Following pins to the bottom, so the last line must be visible.
        assert!(content.contains("line 49"));
        assert!(!content.contains("line 0 "));
    }

    #[test]
    fn draw_shows_waiting_message_when_empty() {
        let mut block = block();

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| block.draw(f, f.area()).unwrap()).unwrap();

        let buffer: &Buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Waiting for logs..."));
    }

    #[test]
    fn draw_shows_ended_marker() {
        let mut block = block();
        block.mark_ended(None);

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| block.draw(f, f.area()).unwrap()).unwrap();

        let buffer: &Buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("stream ended"));
    }
}
