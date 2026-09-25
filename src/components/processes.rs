use crate::filter::{ProcessFilter, ProcessRow};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

pub fn render(
    f: &mut Frame,
    area: Rect,
    app_filter: &ProcessFilter,
    rows: &[ProcessRow],
    selected: usize,
    table_state: &mut TableState,
) {
    let sort_dir = if app_filter.sort_desc() { "▼" } else { "▲" };
    
    let title = if app_filter.is_active() {
        format!(" Process Matrix [SEARCHING]: {}_ ", app_filter.get_query())
    } else if app_filter.has_query() {
        format!(
            " Process Matrix [Filter: \"{}\"] ({} matches, sort: {} {}) ",
            app_filter.get_query(),
            rows.len(),
            app_filter.sort_key().label(),
            sort_dir
        )
    } else {
        format!(
            " Process Matrix ({} total, sort: {} {}) ",
            rows.len(),
            app_filter.sort_key().label(),
            sort_dir
        )
    };

    let header = Row::new(vec!["PID", "USER", "CPU%", "MEM%", "MEM(MB)", "ST", "COMMAND"])
        .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan));

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|p| {
            let cpu_style = if p.cpu_pct > 50.0 {
                Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)
            } else if p.cpu_pct > 15.0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(p.pid.to_string()),
                Cell::from(p.user.clone()),
                Cell::from(format!("{:.1}", p.cpu_pct)).style(cpu_style),
                Cell::from(format!("{:.1}", p.mem_pct)),
                Cell::from(p.mem_mb.to_string()),
                Cell::from(p.state.to_string()),
                Cell::from(p.cmd.clone()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(7),
        Constraint::Length(12),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Length(9),
        Constraint::Length(3),
        Constraint::Min(20),
    ];

    let table = Table::new(table_rows, widths)
        .header(header)
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(237))
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯ ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta))
                .title(title),
        );

    if rows.is_empty() {
        table_state.select(None);
    } else {
        table_state.select(Some(selected.min(rows.len() - 1)));
    }

    f.render_stateful_widget(table, area, table_state);
}