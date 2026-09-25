use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Sparkline},
    Frame,
};

pub fn human_bytes_per_sec(bps: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bps as f64;
    if b >= GB {
        format!("{:.2} GB/s", b / GB)
    } else if b >= MB {
        format!("{:.2} MB/s", b / MB)
    } else if b >= KB {
        format!("{:.1} KB/s", b / KB)
    } else {
        format!("{b:.0} B/s")
    }
}

pub fn render(
    f: &mut Frame,
    area: Rect,
    rx_bps: u64,
    tx_bps: u64,
    rx_hist: &[u64],
    tx_hist: &[u64],
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title(format!(
            " Net ↓{} ↑{} ",
            human_bytes_per_sec(rx_bps),
            human_bytes_per_sec(tx_bps)
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(inner);

    f.render_widget(
        Sparkline::default()
            .block(Block::default().title("RX"))
            .data(rx_hist)
            .style(Style::default().fg(Color::Cyan)),
        split[0],
    );

    if split.len() > 1 {
        f.render_widget(
            Sparkline::default()
                .block(Block::default().title("TX"))
                .data(tx_hist)
                .style(Style::default().fg(Color::LightBlue)),
            split[1],
        );
    }
}