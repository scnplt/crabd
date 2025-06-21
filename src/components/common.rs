use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style, palette::tailwind},
    widgets::{Block, BorderType, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TableStyle {
    pub header_style: Style,
    pub selected_row_style: Style,
    pub row_style: Style,
    pub alt_row_style: Style,
}

impl Default for TableStyle {
    fn default() -> Self {
        let header_style = Style::default().fg(tailwind::SLATE.c200);
        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(tailwind::BLUE.c400);
        let row_style = Style::default()
            .bg(tailwind::SLATE.c800)
            .fg(tailwind::SLATE.c200);
        let alt_row_style = Style::default().bg(tailwind::SLATE.c950);

        Self {
            header_style,
            selected_row_style,
            row_style,
            alt_row_style,
        }
    }
}

pub fn render_scrollbar(frame: &mut Frame, area: Rect, state: &mut ScrollbarState, is_vertical: bool) {
    let (orientation, begin_symbol, end_symbol, track_symbol, thumb_symbol) = if is_vertical {
        (ScrollbarOrientation::VerticalRight, Some("^"), Some("v"), Some("│"), "█")
    } else {
        (ScrollbarOrientation::HorizontalBottom, Some("<"), Some(">"), Some("─"), "■")
    };

    let scrollbar = Scrollbar::new(orientation)
        .begin_symbol(begin_symbol)
        .end_symbol(end_symbol)
        .track_symbol(track_symbol)
        .thumb_symbol(thumb_symbol)
        .style(Style::default().fg(tailwind::BLUE.c900));

    frame.render_stateful_widget(scrollbar, area, state);
}

pub fn render_footer(frame: &mut Frame, area: Rect, text: String, border_style: Option<Style>) {
    let paragraph_style = Style::default().fg(tailwind::SLATE.c200);

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(border_style.unwrap_or(Style::default().fg(tailwind::BLUE.c400)));

    let paragraph = Paragraph::new(text)
        .style(paragraph_style)
        .left_aligned()
        .block(block);

    frame.render_widget(paragraph, area);
}

pub fn time_ago_string(epoch_secs: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    let diff = now - epoch_secs;
    let abs_diff = diff.abs();

    let (value, unit) = if abs_diff >= 31_536_000 {
        (abs_diff / 31_536_000, "year")
    } else if abs_diff >= 2_592_000 {
        (abs_diff / 2_592_000, "month")
    } else if abs_diff >= 86_400 {
        (abs_diff / 86_400, "day")
    } else if abs_diff >= 3_600 {
        (abs_diff / 3_600, "hour")
    } else if abs_diff >= 60 {
        (abs_diff / 60, "minute")
    } else {
        (abs_diff, "second")
    };

    let plural = if value == 1 { "" } else { "s" };

    if diff >= 0 {
        format!("{value} {unit}{plural} ago")
    } else {
        format!("in {value} {unit}{plural}")
    }
}
