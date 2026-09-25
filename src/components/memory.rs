use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Gauge},
    Frame,
};

fn format_bytes(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else {
        format!("{}MB", bytes / MB)
    }
}

pub fn render(
    f: &mut Frame,
    area: Rect,
    mem_used: u64,
    mem_total: u64,
    swap_used: u64,
    swap_total: u64,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Memory ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let mem_ratio = if mem_total > 0 {
        (mem_used as f64 / mem_total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let mem_color = if mem_ratio > 0.85 {
        Color::Red
    } else {
        Color::Yellow
    };

    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(mem_color))
            .ratio(mem_ratio)
            .label(format!("RAM: {}/{}", format_bytes(mem_used), format_bytes(mem_total))),
        split[0],
    );

    let swap_ratio = if swap_total > 0 {
        (swap_used as f64 / swap_total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let swap_color = if swap_ratio > 0.85 {
        Color::Red
    } else {
        Color::Magenta
    };

    if split.len() > 1 {
        f.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(swap_color))
                .ratio(swap_ratio)
                .label(format!("SWP: {}/{}", format_bytes(swap_used), format_bytes(swap_total))),
            split[1],
        );
    }
}