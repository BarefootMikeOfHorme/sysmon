use crate::network::human_bytes_per_sec;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Sparkline},
    Frame,
};

pub fn render(
    f: &mut Frame,
    area: Rect,
    read_bps: u64,
    write_bps: u64,
    read_hist: &[u64],
    write_hist: &[u64],
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Indexed(208)))
        .title(format!(
            " Disk R:{} W:{} ",
            human_bytes_per_sec(read_bps),
            human_bytes_per_sec(write_bps)
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(inner);

    f.render_widget(
        Sparkline::default()
            .block(Block::default().title("Read"))
            .data(read_hist)
            .style(Style::default().fg(Color::Indexed(208))),
        split[0],
    );

    if split.len() > 1 {
        f.render_widget(
            Sparkline::default()
                .block(Block::default().title("Write"))
                .data(write_hist)
                .style(Style::default().fg(Color::LightRed)),
            split[1],
        );
    }
}