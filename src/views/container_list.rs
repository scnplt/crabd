use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, Paragraph},
    Frame,
};
use bollard::models::ContainerSummary;

pub fn render_container_list(frame: &mut Frame, containers: &[ContainerSummary], selected_index: usize, show_all: bool) {
    let size = frame.area();

    /* let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(90),
            Constraint::Percentage(10),
        ].as_ref())
        .split(size);

    let table_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(100),
        ].as_ref())
        .split(chunks[0]); */

    let header = Row::new(vec![
        "ID",
        "Name",
        "Image Name",
        "Status",
        "Ports"
    ]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    /* let header = Row::new(vec![
        Cell::from("ID"),
        Cell::from("Name"),
        Cell::from("Image Name"),
        Cell::from("Uptime"),
        Cell::from("Ports"),
    ]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)); */
    
    let rows: Vec<Row> = containers.iter().enumerate().map(|(index, container)| {
        let style = Style::default();
        if index == selected_index { style.bg(Color::Blue); }

        let ports_text = container.ports.as_ref().map_or("-".to_string(), |ports| {
            let mut unique_ports: Vec<(u16, u16)> = ports.iter()
                .filter(|p| p.public_port.is_some())
                .map(|p| (p.private_port, p.public_port.unwrap()))
                .collect();
            unique_ports.sort_by_key(|&(private, _)| private);
            unique_ports.dedup();
            
            unique_ports.iter()
                .map(|&(private, public)| format!("{}:{}/tcp", private, public))
                .collect::<Vec<String>>()
                .join("\n")
        });
        
        let row_height = ports_text.lines().count() as u16;
        
        Row::new(vec![
            Cell::from(container.id.as_deref().unwrap_or("-").to_string()).style(style),
            Cell::from(container.names.as_ref().map_or("-", |n| &n[0]).to_string()).style(style),
            Cell::from(container.image.as_deref().unwrap_or("-").to_string()).style(style),
            Cell::from(format!("{}s", container.state.as_deref().unwrap_or("-"))).style(style),
            Cell::from(ports_text).style(style),
        ]).height(row_height)
    }).collect();
    
    let table = Table::new(rows, vec![
        Constraint::Percentage(10),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ])
    .header(header)
    .block(Block::default().title("Docker Containers").borders(Borders::ALL));
    
    frame.render_widget(table, size);
}
