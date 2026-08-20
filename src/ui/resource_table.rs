use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Text,
    widgets::{Cell, HighlightSpacing, Row, ScrollbarState, Table, TableState},
};

use crate::{
    event::AppEvent,
    ui::common::{RefreshTicker, TableStyle, render_scrollbar},
};

pub const DEFAULT_ROW_HEIGHT: usize = 3;

pub struct ResourceTableInfo<RowType> {
    pub items: Vec<RowType>,
    pub state: TableState,
    pub row_heights: Vec<usize>,
    pub style: TableStyle,
    pub err: Option<String>,
    pub ticker: RefreshTicker,
    scrollbar_state: ScrollbarState,
    scroll: usize,
}

impl<RowType> Default for ResourceTableInfo<RowType> {
    fn default() -> Self {
        Self {
            items: vec![],
            state: TableState::default().with_selected(0),
            row_heights: vec![],
            style: TableStyle::default(),
            err: None,
            ticker: RefreshTicker::default(),
            scrollbar_state: ScrollbarState::default(),
            scroll: 0,
        }
    }
}

/// A single row of a `ResourceTable`, projected from a source Docker API type.
pub trait ResourceRow: Sized {
    type Source;

    fn from_source(source: &Self::Source) -> Self;

    fn cells(&self) -> Vec<String>;

    fn height(&self) -> usize {
        DEFAULT_ROW_HEIGHT
    }

    /// Sorts the rows in place. Default preserves the API's original order.
    fn sort_rows(_rows: &mut Vec<Self>) {}

    fn from_list(sources: Vec<Self::Source>) -> Vec<Self> {
        let mut rows = sources.iter().map(Self::from_source).collect::<Vec<Self>>();
        Self::sort_rows(&mut rows);
        rows
    }
}

/// Outcome of a resource-specific key handler.
pub enum KeyOutcome {
    /// The key was consumed; optionally emit an `AppEvent`.
    Handled(Option<AppEvent>),
    /// The key was not handled by the resource; fall through to shared navigation handling.
    Fallthrough,
}

pub trait ResourceTable {
    type RowType: ResourceRow;

    const HEADERS: &'static [&'static str];
    const WIDTHS: &'static [Constraint];
    const DEFAULT_FOOTER: &'static str;

    fn table_info(&self) -> &ResourceTableInfo<Self::RowType>;
    fn table_info_mut(&mut self) -> &mut ResourceTableInfo<Self::RowType>;

    fn refresh_event(&self) -> AppEvent;
    fn handle_resource_key_event(&mut self, key_event: KeyEvent) -> Result<KeyOutcome>;
    fn error_message(&self, raw: &str) -> String;

    /// Whether the row is currently visible (rendered/navigable). Default: all rows visible.
    #[allow(unused_variables)]
    fn is_row_visible(&self, row: &Self::RowType) -> bool {
        true
    }

    fn footer_text(&self) -> String {
        Self::DEFAULT_FOOTER.to_string()
    }

    fn after_draw(&mut self) {}

    /// Indices into `table_info().items` for the rows that are currently visible.
    /// Rendering, navigation and selection resolution all share this view.
    fn visible_indices(&self) -> Vec<usize> {
        self.table_info()
            .items
            .iter()
            .enumerate()
            .filter(|(_, row)| self.is_row_visible(row))
            .map(|(index, _)| index)
            .collect()
    }

    // The selection index refers to the rendered (filtered) rows, so it must be
    // resolved and wrapped against the visible list, not `items`.
    fn selected_row(&self) -> Option<&Self::RowType> {
        let index = self.table_info().state.selected()?;
        let item_index = *self.visible_indices().get(index)?;
        self.table_info().items.get(item_index)
    }

    fn next_row(&mut self) {
        let Some(last_index) = self.visible_indices().len().checked_sub(1) else {
            return;
        };
        let next_index = self
            .table_info()
            .state
            .selected()
            .map_or(0, |i| if i >= last_index { 0 } else { i + 1 });
        self.select_row(next_index);
    }

    fn previous_row(&mut self) {
        let Some(last_index) = self.visible_indices().len().checked_sub(1) else {
            return;
        };
        let previous_index = self
            .table_info()
            .state
            .selected()
            .map_or(0, |i| if i == 0 { last_index } else { i - 1 });
        self.select_row(previous_index);
    }

    fn select_row(&mut self, index: usize) {
        let table_info = self.table_info_mut();
        table_info.state.select(Some(index));
        table_info.scroll = table_info.row_heights.iter().take(index).sum();
    }

    fn handle_nav_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        let mut event = None;

        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => event = Some(AppEvent::Quit),
            KeyCode::Down | KeyCode::Char('j') => self.next_row(),
            KeyCode::Up | KeyCode::Char('k') => self.previous_row(),
            _ => {}
        }

        Ok(event)
    }

    fn update_scroll_state(&mut self) {
        let table_info = self.table_info_mut();
        let content_height: usize = table_info.row_heights.iter().sum();
        table_info.scrollbar_state = table_info
            .scrollbar_state
            .content_length(content_height)
            .position(table_info.scroll);
    }

    fn update_with_items(&mut self, items: Vec<Self::RowType>) {
        let table_info = self.table_info_mut();
        let is_empty_before_update = table_info.items.is_empty();
        table_info.items = items;

        if is_empty_before_update && !table_info.items.is_empty() {
            self.select_row(0);
        }
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let rows: Vec<(usize, Vec<String>)> = self
            .visible_indices()
            .iter()
            .map(|&i| {
                let row = &self.table_info().items[i];
                (row.height(), row.cells())
            })
            .collect();

        let heights: Vec<usize> = rows
            .iter()
            .map(|(h, _)| *h)
            .take(rows.len().saturating_sub(1))
            .collect();

        let info = self.table_info_mut();
        info.row_heights = heights;

        let header = Self::HEADERS
            .iter()
            .map(|h| Cell::from(*h))
            .collect::<Row>()
            .style(info.style.header_style)
            .height(1);

        let table_rows = rows
            .into_iter()
            .enumerate()
            .map(|(index, (height, cells))| {
                let row_style = if index % 2 == 0 {
                    info.style.row_style
                } else {
                    info.style.alt_row_style
                };

                cells
                    .into_iter()
                    .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
                    .collect::<Row>()
                    .style(row_style)
                    .height(height as u16)
            });

        let table = Table::new(table_rows, Self::WIDTHS.to_vec())
            .header(header)
            .row_highlight_style(info.style.selected_row_style)
            .highlight_symbol(Text::from(vec!["".into(), " ● ".into()]))
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(table, area, &mut info.state);
    }

    fn render_footer(&mut self, frame: &mut Frame, area: Rect) {
        let mut text = self.footer_text();
        let mut border = None;

        if let Some(err) = &self.table_info().err {
            border = Some(Style::new().red());
            text = err.clone();
        }

        crate::ui::common::render_footer(frame, area, text, border);
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        use Constraint::{Length, Min};

        let vertical_layout = Layout::vertical([Min(0), Length(3)]);
        let [content_area, footer_area] = vertical_layout.areas(area);

        let horizontal_content_layout = Layout::horizontal([Min(0), Length(1)]);
        let [table_area, scrollbar_area] = horizontal_content_layout.areas(content_area);

        self.render_table(frame, table_area);

        self.update_scroll_state();

        let table_info = self.table_info_mut();
        render_scrollbar(frame, scrollbar_area, &mut table_info.scrollbar_state, true);

        self.render_footer(frame, footer_area);

        self.after_draw();

        Ok(())
    }

    fn tick(&mut self) -> Result<Option<AppEvent>> {
        if self.table_info_mut().ticker.should_refresh() {
            Ok(Some(self.refresh_event()))
        } else {
            Ok(None)
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<Option<AppEvent>> {
        if self.table_info().err.is_some() {
            self.table_info_mut().err = None;
            return Ok(None);
        }

        match self.handle_resource_key_event(key_event)? {
            KeyOutcome::Handled(event) => Ok(event),
            KeyOutcome::Fallthrough => self.handle_nav_key_event(key_event),
        }
    }

    fn show_err(&mut self, raw: String) {
        let msg = self.error_message(&raw);
        self.table_info_mut().err = Some(format!("[ERR] {}", msg.trim()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

    #[derive(Default, Clone)]
    struct TestRow {
        label: String,
        visible: bool,
    }

    impl ResourceRow for TestRow {
        type Source = (String, bool);

        fn from_source(source: &Self::Source) -> Self {
            Self {
                label: source.0.clone(),
                visible: source.1,
            }
        }

        fn cells(&self) -> Vec<String> {
            vec![self.label.clone()]
        }
    }

    #[derive(Default)]
    struct TestTable {
        info: ResourceTableInfo<TestRow>,
        filter: bool,
    }

    impl ResourceTable for TestTable {
        type RowType = TestRow;

        const HEADERS: &'static [&'static str] = &["Label"];
        const WIDTHS: &'static [Constraint] = &[Constraint::Min(10)];
        const DEFAULT_FOOTER: &'static str = " footer";

        fn table_info(&self) -> &ResourceTableInfo<Self::RowType> {
            &self.info
        }

        fn table_info_mut(&mut self) -> &mut ResourceTableInfo<Self::RowType> {
            &mut self.info
        }

        fn refresh_event(&self) -> AppEvent {
            AppEvent::UpdateContainers
        }

        fn handle_resource_key_event(&mut self, key_event: KeyEvent) -> Result<KeyOutcome> {
            match key_event.code {
                KeyCode::Char('h') => Ok(KeyOutcome::Handled(None)),
                _ => Ok(KeyOutcome::Fallthrough),
            }
        }

        fn error_message(&self, raw: &str) -> String {
            raw.to_string()
        }

        fn is_row_visible(&self, row: &Self::RowType) -> bool {
            !self.filter || row.visible
        }
    }

    fn rows(labels: &[(&str, bool)]) -> Vec<TestRow> {
        labels
            .iter()
            .map(|(l, v)| TestRow {
                label: (*l).to_string(),
                visible: *v,
            })
            .collect()
    }

    #[test]
    fn next_and_previous_row_wrap_around_visible_rows_only() {
        let mut table = TestTable {
            filter: true,
            ..Default::default()
        };
        table.update_with_items(rows(&[("a", true), ("b", false), ("c", true)]));
        // row heights are populated by render normally; set manually for this test.
        table.info.row_heights = vec![DEFAULT_ROW_HEIGHT, DEFAULT_ROW_HEIGHT];

        table.select_row(0);
        assert_eq!(
            table.selected_row().map(|r| r.label.clone()),
            Some("a".to_string())
        );

        table.next_row();
        // Only "a" and "c" are visible, so index 1 resolves to "c".
        assert_eq!(
            table.selected_row().map(|r| r.label.clone()),
            Some("c".to_string())
        );

        table.next_row();
        assert_eq!(
            table.selected_row().map(|r| r.label.clone()),
            Some("a".to_string())
        );

        table.previous_row();
        assert_eq!(
            table.selected_row().map(|r| r.label.clone()),
            Some("c".to_string())
        );
    }

    #[test]
    fn select_row_updates_scroll_from_row_heights() {
        let mut table = TestTable::default();
        table.update_with_items(rows(&[("a", true), ("b", true), ("c", true)]));
        table.info.row_heights = vec![3, 5, 7];

        table.select_row(2);
        assert_eq!(table.info.scroll, 8);
    }

    #[test]
    fn handle_key_event_clears_error_without_dispatch() {
        let mut table = TestTable::default();
        table.info.err = Some("boom".to_string());

        let result = table
            .handle_key_event(KeyEvent::from(KeyCode::Char('h')))
            .unwrap();
        assert!(result.is_none());
        assert!(table.info.err.is_none());
    }

    #[test]
    fn handled_outcome_does_not_reach_nav_handling() {
        let mut table = TestTable::default();
        table.update_with_items(rows(&[("a", true), ("b", true)]));
        table.info.row_heights = vec![DEFAULT_ROW_HEIGHT];
        table.select_row(0);

        table
            .handle_key_event(KeyEvent::from(KeyCode::Char('h')))
            .unwrap();
        // 'h' is Handled(None) for TestTable, so selection must not move.
        assert_eq!(table.info.state.selected(), Some(0));
    }

    #[test]
    fn fallthrough_outcome_reaches_nav_handling() {
        let mut table = TestTable::default();
        table.update_with_items(rows(&[("a", true), ("b", true)]));
        table.info.row_heights = vec![DEFAULT_ROW_HEIGHT];
        table.select_row(0);

        let event = table
            .handle_key_event(KeyEvent::from(KeyCode::Char('j')))
            .unwrap();
        assert!(event.is_none());
        assert_eq!(table.info.state.selected(), Some(1));

        let event = table
            .handle_key_event(KeyEvent::from(KeyCode::Char('q')))
            .unwrap();
        assert!(matches!(event, Some(AppEvent::Quit)));
    }

    #[test]
    fn update_with_items_selects_first_row_only_on_empty_to_nonempty_transition() {
        let mut table = TestTable::default();
        table.info.state.select(None);

        table.update_with_items(rows(&[("a", true)]));
        assert_eq!(table.info.state.selected(), Some(0));

        table.select_row(0);
        table.info.state.select(None);
        table.update_with_items(rows(&[("a", true), ("b", true)]));
        // Items were already non-empty before this update, so selection stays untouched.
        assert_eq!(table.info.state.selected(), None);
    }

    #[test]
    fn draw_renders_headers_and_selection_marker() {
        let mut table = TestTable::default();
        table.update_with_items(rows(&[("alpha", true), ("beta", true), ("gamma", true)]));
        table.select_row(0);

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| table.draw(f, f.area()).unwrap()).unwrap();

        let buffer: &Buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        assert!(content.contains("Label"));
        assert!(content.contains(" ● "));
        // 3 visible rows -> last row's height is not recorded.
        assert_eq!(table.info.row_heights.len(), 2);
    }
}
