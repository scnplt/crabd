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

    #[test]
    fn error_message_captures_daemon_in_use_segment() {
        let table = NetworkTable::default();
        let raw = "Error response from daemon: error while removing network: network foo id \
            abc123def456 has active endpoints";
        assert_eq!(
            table.error_message(raw),
            "network foo id abc123def456 has active endpoints"
        );

        assert_eq!(
            table.error_message("no colons here"),
            "Something went wrong..."
        );
    }

    #[test]
    fn from_source_strips_fractional_seconds_and_truncates_id() {
        let mut net = network("abcdef012345extra", "my-net");
        net.created = Some("2024-01-02T03:04:05.123456789Z".to_string());

        let row = NetworkTableRow::from_source(&net);
        assert_eq!(row.created_at, "2024-01-02T03:04:05Z");
        assert_eq!(row.id, "abcdef012345...");
    }

    #[test]
    fn handle_resource_key_event_dispatches_remove_network() {
        let mut table = NetworkTable::default();
        table.update_with_items(NetworkTableRow::from_list(vec![network(
            "111111111111",
            "my-net",
        )]));
        table.select_row(0);

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Delete))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RemoveNetwork(name))) => {
                assert_eq!(name, "my-net")
            }
            _ => panic!("expected RemoveNetwork via Delete"),
        }

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('d')))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RemoveNetwork(name))) => {
                assert_eq!(name, "my-net")
            }
            _ => panic!("expected RemoveNetwork via 'd'"),
        }

        assert!(matches!(
            table
                .handle_resource_key_event(KeyEvent::from(KeyCode::Char('z')))
                .unwrap(),
            KeyOutcome::Fallthrough
        ));
    }
}
