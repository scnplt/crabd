use bollard::secret::Network;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Constraint;
use regex::Regex;

use crate::{
    event::AppEvent,
    ui::resource_table::{KeyOutcome, ResourceRow, ResourceTable, ResourceTableInfo},
};

const REGEX_NETWORK_IN_USE: &str = r":(?:[^:]+:)?\s*([^\(]+)";
const REGEX_NETWORK_CREATED_AT: &str = r"\.\d+";

#[derive(Default)]
pub struct NetworkTable {
    info: ResourceTableInfo<NetworkTableRow>,
}

#[derive(Default)]
pub struct NetworkTableRow {
    id: String,
    name: String,
    driver: String,
    created_at: String,
}

impl ResourceTable for NetworkTable {
    type RowType = NetworkTableRow;

    const HEADERS: &'static [&'static str] = &["ID", "Name", "Driver", "Created At"];
    const WIDTHS: &'static [Constraint] = &[
        Constraint::Length(15),
        Constraint::Min(15),
        Constraint::Min(0),
        Constraint::Length(27),
    ];
    const DEFAULT_FOOTER: &'static str = " <Del/D> remove";

    fn table_info(&self) -> &ResourceTableInfo<Self::RowType> {
        &self.info
    }

    fn table_info_mut(&mut self) -> &mut ResourceTableInfo<Self::RowType> {
        &mut self.info
    }

    fn refresh_event(&self) -> AppEvent {
        AppEvent::UpdateNetworks
    }

    fn handle_resource_key_event(&mut self, key_event: KeyEvent) -> Result<KeyOutcome> {
        let outcome = match key_event.code {
            KeyCode::Delete | KeyCode::Char('d') => KeyOutcome::Handled(
                self.selected_row()
                    .map(|n| AppEvent::RemoveNetwork(n.name.clone())),
            ),
            _ => KeyOutcome::Fallthrough,
        };

        Ok(outcome)
    }

    fn error_message(&self, raw: &str) -> String {
        Regex::new(REGEX_NETWORK_IN_USE)
            .ok()
            .and_then(|re| re.captures(raw))
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Something went wrong...".to_string())
    }
}

impl ResourceRow for NetworkTableRow {
    type Source = Network;

    fn from_source(network: &Self::Source) -> Self {
        let id = format!("{}...", &network.id.as_deref().unwrap_or("-")[..12]);

        let raw_created = network.created.as_deref().unwrap_or_default();
        let re = Regex::new(REGEX_NETWORK_CREATED_AT).unwrap();
        let created_at = re.replace(raw_created, "").to_string();

        Self {
            id,
            name: network.name.as_deref().unwrap_or("-").to_string(),
            driver: network.driver.as_deref().unwrap_or("-").to_string(),
            created_at,
        }
    }

    fn cells(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.name.clone(),
            self.driver.clone(),
            self.created_at.clone(),
        ]
    }

    fn sort_rows(rows: &mut Vec<Self>) {
        rows.sort_by_key(|n| n.name.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(id: &str, name: &str) -> Network {
        Network {
            id: Some(id.to_string()),
            name: Some(name.to_string()),
            driver: Some("bridge".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn from_list_sorts_by_name() {
        let networks = vec![
            network("111111111111", "charlie"),
            network("222222222222", "alpha"),
            network("333333333333", "bravo"),
        ];
        let rows = NetworkTableRow::from_list(networks);
        let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();

        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }
}
