use bollard::secret::Volume;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Constraint;

use crate::{
    event::AppEvent,
    ui::resource_table::{KeyOutcome, ResourceRow, ResourceTable, ResourceTableInfo},
};

#[derive(Default)]
pub struct VolumeTable {
    info: ResourceTableInfo<VolumeTableRow>,
}

#[derive(Default)]
pub struct VolumeTableRow {
    name: String,
    driver: String,
    created_at: String,
}

impl ResourceTable for VolumeTable {
    type RowType = VolumeTableRow;

    const HEADERS: &'static [&'static str] = &["Name", "Driver", "Created At"];
    const WIDTHS: &'static [Constraint] = &[
        Constraint::Min(30),
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
        AppEvent::UpdateVolumes
    }

    fn handle_resource_key_event(&mut self, key_event: KeyEvent) -> Result<KeyOutcome> {
        let outcome = match key_event.code {
            KeyCode::Delete | KeyCode::Char('d') => KeyOutcome::Handled(
                self.selected_row()
                    .map(|v| AppEvent::RemoveVolume(v.name.clone(), false)),
            ),
            KeyCode::Char('f') => KeyOutcome::Handled(
                self.selected_row()
                    .map(|v| AppEvent::RemoveVolume(v.name.clone(), true)),
            ),
            _ => KeyOutcome::Fallthrough,
        };

        Ok(outcome)
    }
}

impl ResourceRow for VolumeTableRow {
    type Source = Volume;

    fn from_source(volume: &Self::Source) -> Self {
        Self {
            name: volume.name.clone(),
            driver: volume.driver.clone(),
            created_at: volume.created_at.as_deref().unwrap_or_default().to_string(),
        }
    }

    fn cells(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.driver.clone(),
            self.created_at.clone(),
        ]
    }

    fn sort_rows(rows: &mut Vec<Self>) {
        rows.sort_by_key(|v| v.name.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(name: &str) -> Volume {
        Volume {
            name: name.to_string(),
            driver: "local".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn from_list_sorts_by_name() {
        let volumes = vec![volume("charlie"), volume("alpha"), volume("bravo")];
        let rows = VolumeTableRow::from_list(volumes);
        let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();

        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn handle_resource_key_event_dispatches_expected_events() {
        let mut table = VolumeTable::default();
        table.update_with_items(VolumeTableRow::from_list(vec![volume("my-volume")]));
        table.select_row(0);

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('d')))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RemoveVolume(name, false))) => {
                assert_eq!(name, "my-volume")
            }
            _ => panic!("expected RemoveVolume(name, false)"),
        }

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('f')))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RemoveVolume(name, true))) => {
                assert_eq!(name, "my-volume")
            }
            _ => panic!("expected RemoveVolume(name, true)"),
        }
    }
}
