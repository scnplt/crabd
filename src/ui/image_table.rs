use bollard::secret::ImageSummary;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Constraint;

use crate::{
    event::AppEvent,
    ui::{
        common::time_ago_string,
        resource_table::{
            DEFAULT_ROW_HEIGHT, KeyOutcome, ResourceRow, ResourceTable, ResourceTableInfo,
        },
    },
};

#[derive(Default)]
pub struct ImageTable {
    info: ResourceTableInfo<ImageTableRow>,
}

#[derive(Default)]
pub struct ImageTableRow {
    id: String,
    tags: String,
    size: String,
    created: String,
    created_epoch: i64,
}

impl ResourceTable for ImageTable {
    type RowType = ImageTableRow;

    const HEADERS: &'static [&'static str] = &["ID", "Tags", "Size", "Created"];
    const WIDTHS: &'static [Constraint] = &[
        Constraint::Length(15),
        Constraint::Min(15),
        Constraint::Min(0),
        Constraint::Length(27),
    ];
    const DEFAULT_FOOTER: &'static str = " <Del/D> remove | <F> force remove";

    fn table_info(&self) -> &ResourceTableInfo<Self::RowType> {
        &self.info
    }

    fn table_info_mut(&mut self) -> &mut ResourceTableInfo<Self::RowType> {
        &mut self.info
    }

    fn refresh_event(&self) -> AppEvent {
        AppEvent::UpdateImages
    }

    fn handle_resource_key_event(&mut self, key_event: KeyEvent) -> Result<KeyOutcome> {
        let outcome = match key_event.code {
            KeyCode::Delete | KeyCode::Char('d') => KeyOutcome::Handled(
                self.selected_row()
                    .map(|i| AppEvent::RemoveImage(i.id.clone(), false)),
            ),
            KeyCode::Char('f') => KeyOutcome::Handled(
                self.selected_row()
                    .map(|i| AppEvent::RemoveImage(i.id.clone(), true)),
            ),
            _ => KeyOutcome::Fallthrough,
        };

        Ok(outcome)
    }
}

impl ResourceRow for ImageTableRow {
    type Source = ImageSummary;

    fn from_source(image: &Self::Source) -> Self {
        let id = image.id.split(":").collect::<Vec<&str>>()[1].to_string();

        Self {
            id,
            tags: image.repo_tags.join("\n"),
            size: image.size.to_string(),
            created: time_ago_string(image.created),
            created_epoch: image.created,
        }
    }

    fn cells(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.tags.clone(),
            self.size.clone(),
            self.created.clone(),
        ]
    }

    fn height(&self) -> usize {
        let tags = self.tags.split('\n').filter(|s| !s.is_empty()).count();
        if tags == 0 {
            DEFAULT_ROW_HEIGHT
        } else {
            tags + 2
        }
    }

    fn sort_rows(rows: &mut Vec<Self>) {
        rows.sort_by_key(|r| r.created_epoch);
        rows.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(id: &str, created: i64) -> ImageSummary {
        ImageSummary {
            id: format!("sha256:{id}"),
            created,
            ..Default::default()
        }
    }

    #[test]
    fn from_list_sorts_newest_first() {
        let images = vec![image("a", 100), image("b", 300), image("c", 200)];
        let rows = ImageTableRow::from_list(images);
        let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();

        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn height_reflects_tag_count() {
        let no_tags = ImageTableRow {
            id: "id".to_string(),
            tags: String::new(),
            size: "0".to_string(),
            created: "now".to_string(),
            created_epoch: 0,
        };
        assert_eq!(no_tags.height(), DEFAULT_ROW_HEIGHT);

        let two_tags = ImageTableRow {
            tags: "repo:tag1\nrepo:tag2".to_string(),
            ..no_tags
        };
        assert_eq!(two_tags.height(), 4);
    }

    #[test]
    fn handle_resource_key_event_dispatches_expected_events() {
        let mut table = ImageTable::default();
        let images = vec![image(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd",
            100,
        )];
        table.update_with_items(ImageTableRow::from_list(images));
        table.select_row(0);
        let id = table.selected_row().unwrap().id.clone();

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('d')))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RemoveImage(got, false))) => assert_eq!(got, id),
            _ => panic!("expected RemoveImage(id, false)"),
        }

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Delete))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RemoveImage(got, false))) => assert_eq!(got, id),
            _ => panic!("expected RemoveImage(id, false)"),
        }

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('f')))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RemoveImage(got, true))) => assert_eq!(got, id),
            _ => panic!("expected RemoveImage(id, true)"),
        }

        assert!(matches!(
            table
                .handle_resource_key_event(KeyEvent::from(KeyCode::Char('z')))
                .unwrap(),
            KeyOutcome::Fallthrough
        ));
    }
}
