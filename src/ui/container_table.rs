use crate::docker::models::{PortMapping, parse_container_state, state_label};
use crate::ui::resource_table::{
    DEFAULT_ROW_HEIGHT, KeyOutcome, ResourceRow, ResourceTable, ResourceTableInfo,
};
use crate::{event::AppEvent, utils::is_container_running};

use bollard::secret::{ContainerStateStatusEnum, ContainerSummary};
use color_eyre::Result;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::Constraint,
};

pub struct ContainerTable {
    show_all: bool,
    info: ResourceTableInfo<ContainerTableRow>,
}

#[derive(PartialEq)]
pub struct ContainerTableRow {
    id: String,
    name: String,
    image: String,
    state: ContainerStateStatusEnum,
    ports: Vec<PortMapping>,
}

impl Default for ContainerTable {
    fn default() -> Self {
        Self {
            show_all: true,
            info: ResourceTableInfo::default(),
        }
    }
}

impl ResourceTable for ContainerTable {
    type RowType = ContainerTableRow;

    const HEADERS: &'static [&'static str] = &["ID", "Name", "Image", "State", "Ports"];
    const WIDTHS: &'static [Constraint] = &[
        Constraint::Length(12),
        Constraint::Percentage(20),
        Constraint::Percentage(30),
        Constraint::Percentage(10),
        Constraint::Min(15),
    ];
    const DEFAULT_FOOTER: &'static str = "";

    fn table_info(&self) -> &ResourceTableInfo<Self::RowType> {
        &self.info
    }

    fn table_info_mut(&mut self) -> &mut ResourceTableInfo<Self::RowType> {
        &mut self.info
    }

    fn refresh_event(&self) -> AppEvent {
        AppEvent::UpdateContainers
    }

    fn is_row_visible(&self, row: &Self::RowType) -> bool {
        self.show_all || is_container_running(row.state)
    }

    fn footer_text(&self) -> String {
        let is_selected_container_running =
            self.selected_row().map(|c| is_container_running(c.state));
        get_footer_text(self.show_all, is_selected_container_running)
    }

    fn after_draw(&mut self) {
        // If there is items but no row selected, select the first row.
        // This happens when changing the `self.show_all` parameter.
        if self.table_info().state.selected().is_none() && !self.visible_indices().is_empty() {
            self.select_row(0);
        }
    }

    fn handle_resource_key_event(&mut self, key_event: KeyEvent) -> Result<KeyOutcome> {
        let outcome = match key_event.code {
            KeyCode::Char('t') => {
                self.show_all = !self.show_all;
                // The old selection index may point past the new visible list; clamp it.
                if let Some(last_index) = self.visible_indices().len().checked_sub(1) {
                    let selected = self.table_info().state.selected().unwrap_or(0);
                    self.select_row(selected.min(last_index));
                }
                KeyOutcome::Handled(None)
            }
            KeyCode::Delete | KeyCode::Char('d') => KeyOutcome::Handled(
                self.selected_row()
                    .map(|c| AppEvent::RemoveContainer(c.id.clone())),
            ),
            KeyCode::Enter => KeyOutcome::Handled(
                self.selected_row()
                    .map(|c| AppEvent::GoToContainerDetails(c.id.clone())),
            ),
            KeyCode::Char(c) => match (c, self.selected_row().map(|c| c.id.clone())) {
                ('r', Some(id)) => KeyOutcome::Handled(Some(AppEvent::RestartContainer(id))),
                ('s', Some(id)) => KeyOutcome::Handled(Some(AppEvent::StopContainer(id))),
                ('x', Some(id)) => KeyOutcome::Handled(Some(AppEvent::KillContainer(id))),
                _ => KeyOutcome::Fallthrough,
            },
            _ => KeyOutcome::Fallthrough,
        };

        Ok(outcome)
    }
}

impl ResourceRow for ContainerTableRow {
    type Source = ContainerSummary;

    fn from_source(container: &Self::Source) -> Self {
        let name: String = container
            .names
            .as_deref()
            .and_then(|names| names.first())
            .and_then(|name| name.strip_prefix("/"))
            .map_or("NaN".to_string(), |name| name.to_string());

        Self {
            id: container.id.as_deref().unwrap_or("-").to_string(),
            name,
            image: container.image.as_deref().unwrap_or("-").to_string(),
            state: parse_container_state(container.state.as_deref()),
            ports: container
                .ports
                .as_deref()
                .map(PortMapping::from_summary_ports)
                .unwrap_or_default(),
        }
    }

    fn cells(&self) -> Vec<String> {
        let ports_text = if self.ports.is_empty() {
            "-".to_string()
        } else {
            self.ports
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("\n")
        };

        vec![
            self.id.clone(),
            self.name.clone(),
            self.image.clone(),
            state_label(self.state),
            ports_text,
        ]
    }

    fn height(&self) -> usize {
        if self.ports.is_empty() {
            DEFAULT_ROW_HEIGHT
        } else {
            self.ports.len() + 2
        }
    }

    fn sort_rows(rows: &mut Vec<Self>) {
        rows.sort_by(|p, n| {
            let p_is_running = p.state == ContainerStateStatusEnum::RUNNING;
            let n_is_running = n.state == ContainerStateStatusEnum::RUNNING;

            match (p_is_running, n_is_running) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                // Tie-break equal states by name so row order does not depend
                // on the order the daemon returned the containers in.
                _ => p
                    .state
                    .as_ref()
                    .cmp(n.state.as_ref())
                    .then_with(|| p.name.cmp(&n.name)),
            }
        });
    }
}

fn get_footer_text(show_all: bool, is_running: Option<bool>) -> String {
    let toggle_text = if show_all { "All" } else { "Running" };
    let mut op_text = "".to_string();

    if let Some(running) = is_running {
        let running_text = if running {
            "restart | <S> stop | <X> kill "
        } else {
            "start "
        };
        op_text = format!(" | <R> {running_text}| <Del/D> remove");
    }

    format!(" <Ent> details | <T> {toggle_text}{op_text}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::secret::PortTypeEnum;

    fn summary_with_state(state: &str) -> ContainerSummary {
        ContainerSummary {
            id: Some(format!("id-{state}")),
            names: Some(vec![format!("/{state}")]),
            image: Some("image".to_string()),
            state: Some(state.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn from_list_sorts_running_first_and_others_alphabetically_by_state() {
        let containers = vec![
            summary_with_state("exited"),
            summary_with_state("restarting"),
            summary_with_state("created"),
            summary_with_state("running"),
            summary_with_state("paused"),
        ];

        let rows = ContainerTableRow::from_list(containers);
        let states: Vec<ContainerStateStatusEnum> = rows.iter().map(|r| r.state).collect();

        // Running must come first.
        assert_eq!(states[0], ContainerStateStatusEnum::RUNNING);
        // "restarting" is not "running", so it must not be treated as running.
        assert_ne!(states[1], ContainerStateStatusEnum::RESTARTING);

        // Remaining rows are ordered alphabetically by state name:
        // created < exited < paused < restarting.
        let non_running_states: Vec<ContainerStateStatusEnum> = states[1..].to_vec();
        assert_eq!(
            non_running_states,
            vec![
                ContainerStateStatusEnum::CREATED,
                ContainerStateStatusEnum::EXITED,
                ContainerStateStatusEnum::PAUSED,
                ContainerStateStatusEnum::RESTARTING,
            ]
        );
    }

    #[test]
    fn from_list_breaks_equal_state_ties_by_name() {
        let named = |name: &str| ContainerSummary {
            id: Some(format!("id-{name}")),
            names: Some(vec![format!("/{name}")]),
            state: Some("exited".to_string()),
            ..Default::default()
        };

        let names_of = |rows: &[ContainerTableRow]| -> Vec<String> {
            rows.iter().map(|r| r.name.clone()).collect()
        };

        // Both input orders must yield the same on-screen order.
        let one_way = ContainerTableRow::from_list(vec![named("bravo"), named("alpha")]);
        let other_way = ContainerTableRow::from_list(vec![named("alpha"), named("bravo")]);

        assert_eq!(names_of(&one_way), names_of(&other_way));
        assert_eq!(names_of(&one_way), vec!["alpha", "bravo"]);
    }

    #[test]
    fn from_strips_leading_slash_from_name() {
        let container = summary_with_state("running");
        let row = ContainerTableRow::from_source(&container);
        assert_eq!(row.name, "running");
    }

    #[test]
    fn cells_fall_back_to_dash_for_missing_fields() {
        let container = ContainerSummary::default();
        let row = ContainerTableRow::from_source(&container);
        let cells = row.cells();

        assert_eq!(cells[0], "-"); // id
        assert_eq!(cells[2], "-"); // image
        assert_eq!(cells[3], "-"); // state (EMPTY)
        assert_eq!(cells[4], "-"); // ports
    }

    #[test]
    fn get_footer_text_variants() {
        let text = get_footer_text(true, None);
        assert_eq!(text, " <Ent> details | <T> All");

        let text = get_footer_text(true, Some(true));
        assert!(text.contains("restart | <S> stop | <X> kill"));
        assert!(text.contains("<T> All"));

        let text = get_footer_text(false, Some(false));
        assert!(text.contains("<R> start"));
        assert!(text.contains("<T> Running"));
    }

    fn summary_with_ports(id: &str, state: &str) -> ContainerSummary {
        ContainerSummary {
            id: Some(id.to_string()),
            names: Some(vec![format!("/{id}")]),
            image: Some("image".to_string()),
            state: Some(state.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn handle_resource_key_event_dispatches_expected_events() {
        let mut table = ContainerTable::default();
        let rows = ContainerTableRow::from_list(vec![
            summary_with_ports("running-id", "running"),
            summary_with_ports("exited-id", "exited"),
        ]);
        table.update_with_items(rows);
        table.select_row(0);

        let id = table.selected_row().unwrap().id.clone();

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('r')))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RestartContainer(got))) => assert_eq!(got, id),
            _ => panic!("expected RestartContainer"),
        }

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('s')))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::StopContainer(got))) => assert_eq!(got, id),
            _ => panic!("expected StopContainer"),
        }

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('x')))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::KillContainer(got))) => assert_eq!(got, id),
            _ => panic!("expected KillContainer"),
        }

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Enter))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::GoToContainerDetails(got))) => assert_eq!(got, id),
            _ => panic!("expected GoToContainerDetails"),
        }

        match table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Delete))
            .unwrap()
        {
            KeyOutcome::Handled(Some(AppEvent::RemoveContainer(got))) => assert_eq!(got, id),
            _ => panic!("expected RemoveContainer"),
        }
    }

    #[test]
    fn esc_and_arrow_keys_fall_through_to_shared_navigation() {
        let mut table = ContainerTable::default();
        let rows = ContainerTableRow::from_list(vec![
            summary_with_ports("running-id", "running"),
            summary_with_ports("other-id", "running"),
        ]);
        table.update_with_items(rows);
        table.info.row_heights = vec![DEFAULT_ROW_HEIGHT, DEFAULT_ROW_HEIGHT];
        table.select_row(0);

        for code in [KeyCode::Esc, KeyCode::Up, KeyCode::Down] {
            match table
                .handle_resource_key_event(KeyEvent::from(code))
                .unwrap()
            {
                KeyOutcome::Fallthrough => {}
                _ => panic!("expected {code:?} to fall through to shared navigation"),
            }
        }

        // End-to-end through the shared handler: Esc quits, arrows move the selection.
        let event = table
            .handle_key_event(KeyEvent::from(KeyCode::Esc))
            .unwrap();
        assert!(matches!(event, Some(AppEvent::Quit)));

        table
            .handle_key_event(KeyEvent::from(KeyCode::Down))
            .unwrap();
        assert_eq!(table.table_info().state.selected(), Some(1));

        table.handle_key_event(KeyEvent::from(KeyCode::Up)).unwrap();
        assert_eq!(table.table_info().state.selected(), Some(0));
    }

    #[test]
    fn toggle_show_all_shrinks_visible_rows_and_clamps_selection() {
        let mut table = ContainerTable::default();
        let rows = ContainerTableRow::from_list(vec![
            summary_with_ports("running-id", "running"),
            summary_with_ports("exited-id", "exited"),
        ]);
        table.update_with_items(rows);
        table.info.row_heights = vec![DEFAULT_ROW_HEIGHT];

        assert_eq!(table.visible_indices().len(), 2);

        // Select the last index before toggling, then verify it gets clamped.
        table.select_row(1);

        table
            .handle_resource_key_event(KeyEvent::from(KeyCode::Char('t')))
            .unwrap();

        assert_eq!(table.visible_indices().len(), 1);
        let selected = table.table_info().state.selected().unwrap();
        assert!(selected < table.visible_indices().len());
    }

    #[test]
    fn height_reflects_port_count() {
        let no_ports = ContainerTableRow {
            id: "id".to_string(),
            name: "name".to_string(),
            image: "image".to_string(),
            state: ContainerStateStatusEnum::RUNNING,
            ports: vec![],
        };
        assert_eq!(no_ports.height(), DEFAULT_ROW_HEIGHT);

        let two_ports = ContainerTableRow {
            ports: vec![
                PortMapping {
                    private: 80,
                    public: 8080,
                    protocol: PortTypeEnum::TCP,
                },
                PortMapping {
                    private: 443,
                    public: 8443,
                    protocol: PortTypeEnum::TCP,
                },
            ],
            ..no_ports
        };
        assert_eq!(two_ports.height(), 4);
    }

    #[test]
    fn cells_joins_multiple_ports_with_newline() {
        let row = ContainerTableRow {
            id: "id".to_string(),
            name: "name".to_string(),
            image: "image".to_string(),
            state: ContainerStateStatusEnum::RUNNING,
            ports: vec![
                PortMapping {
                    private: 80,
                    public: 8080,
                    protocol: PortTypeEnum::TCP,
                },
                PortMapping {
                    private: 443,
                    public: 8443,
                    protocol: PortTypeEnum::TCP,
                },
            ],
        };

        let cells = row.cells();
        assert!(cells[4].contains('\n'));
        assert_eq!(cells[4].matches('\n').count(), 1);
    }
}
