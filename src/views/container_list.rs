use bollard::{models::ContainerSummary, secret::Port};
use ratatui::{
    Frame,
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

pub fn render_container_list(
    frame: &mut Frame,
    containers: &[ContainerSummary],
    selected_index: usize,
) {
    let size = frame.area();

    let header = Row::new(vec!["ID", "Name", "Image", "State", "Ports"]).style(
        Style::default()
            .fg(Color::Black)
            .bg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = containers
        .iter()
        .enumerate()
        .map(|(index, container)| {
            let mut style = Style::default();

            if index == selected_index {
                style = style.bg(Color::Blue);
            }

            let ports_text = container.ports.as_ref().map_or("-".to_string(), get_ports_text);

            let port_counts = ports_text.lines().count() as u16;
            let row_height = if port_counts > 0 { port_counts } else { 1 };

            Row::new(vec![
                Cell::from(container.id.as_deref().unwrap_or("-").to_string()).style(style),
                Cell::from(container.names.as_ref().map_or("-", |n| &n[0]).to_string())
                    .style(style),
                Cell::from(container.image.as_deref().unwrap_or("-").to_string()).style(style),
                Cell::from(format!("{}s", container.state.as_deref().unwrap_or("-"))).style(style),
                Cell::from(ports_text).style(style),
            ])
            .height(row_height)
        })
        .collect();

    let table = Table::new(
        rows,
        vec![
            Constraint::Percentage(10),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("Docker Containers")
            .borders(Borders::ALL),
    );

    frame.render_widget(table, size);
}

fn get_ports_text(ports: &Vec<Port>) -> String {
    let mut filtered_ports: Vec<(u16, u16)> = ports.iter()
        .filter(|p| p.public_port.is_some())
        .map(|p| (p.private_port, p.public_port.unwrap()))
        .collect();

    filtered_ports.sort_by_key(|&(private, _)| private);
    filtered_ports.dedup();
    
    filtered_ports.iter()
        .map(|&(private, public)| format!("{}:{}", private, public))
        .collect::<Vec<String>>()
        .join("\n")
}