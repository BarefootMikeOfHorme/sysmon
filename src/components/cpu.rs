use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, Sparkline},
    Frame,
};

pub fn render(f: &mut Frame, area: Rect, global_pct: f32, history: &[u64], per_core: &[f32]) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: CPU Load Sparkline
    let cpu_box = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(format!(" CPU Load ({:.1}%) ", global_pct));

    let inner_cpu = cpu_box.inner(split[0]);
    f.render_widget(cpu_box, split[0]);

    if inner_cpu.height >= 2 {
        let wave_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(inner_cpu);

        let top_spark = Sparkline::default()
            .data(history)
            .max(100)
            .style(Style::default().fg(Color::LightGreen));

        let bottom_spark = Sparkline::default()
            .data(history)
            .max(100)
            .style(Style::default().fg(Color::DarkGray));

        f.render_widget(top_spark, wave_split[0]);
        f.render_widget(bottom_spark, wave_split[1]);
    }

    // Right: Per-Core Gauges
    let core_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(format!(" Cores ({}) ", per_core.len()));

    let inner = core_block.inner(split[1]);
    f.render_widget(core_block, split[1]);

    let n = per_core.len();
    if n > 0 && inner.height > 0 && inner.width > 0 {
        let cols_per_row = (inner.width as usize / 12).clamp(1, 8);
        let rows_needed = (n + cols_per_row - 1) / cols_per_row;
        let max_rows = (inner.height as usize).min(rows_needed);

        let row_constraints: Vec<Constraint> = (0..max_rows)
            .map(|_| Constraint::Length(1))
            .collect();

        let row_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(inner);

        for (row_idx, row_area) in row_areas.iter().enumerate() {
            let start = row_idx * cols_per_row;
            let end = (start + cols_per_row).min(n);
            if start >= end {
                continue;
            }

            let count = end - start;
            let seg_constraints: Vec<Constraint> = (0..count)
                .map(|_| Constraint::Ratio(1, count as u32))
                .collect();

            let segs = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(seg_constraints)
                .split(*row_area);

            for (i, core_idx) in (start..end).enumerate() {
                let pct = per_core[core_idx];
                let color = if pct > 85.0 {
                    Color::Red
                } else if pct > 60.0 {
                    Color::Yellow
                } else {
                    Color::Green
                };

                let label = format!("C{:<2}{:>3.0}%", core_idx, pct);
                let gauge = Gauge::default()
                    .gauge_style(Style::default().fg(color))
                    .label(label)
                    .ratio((pct.clamp(0.0, 100.0) / 100.0) as f64);

                if i < segs.len() {
                    f.render_widget(gauge, segs[i]);
                }
            }
        }
    }
}