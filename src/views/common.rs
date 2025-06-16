use ratatui::{
    buffer::Buffer, 
    layout::Rect, 
    style::{palette::tailwind, Style}, 
    widgets::{Block, BorderType, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget}, 
    Frame
};

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

pub fn render_footer(area: Rect, buf: &mut Buffer, text: String, border_style: Option<Style>, style: Option<Style>) {
    let border_style = border_style.unwrap_or(Style::default().fg(tailwind::BLUE.c400));
    let footer_style = style.unwrap_or(Style::default().fg(tailwind::SLATE.c200));

    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(border_style);

    Paragraph::new(text)
        .style(footer_style)
        .left_aligned()
        .block(block)
        .render(area, buf);
}
