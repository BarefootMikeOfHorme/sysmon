use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use filter::{ProcessFilter, ProcessRow};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table, TableState},
    Frame, Terminal,
};
use std::{
    collections::VecDeque,
    error::Error,
    io, panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Disks, Networks, System};

mod filter;

const HISTORY_LEN: usize = 90;
const TICK_MS: u64 = 400;

#[derive(Clone, Default)]
struct History {
    buf: VecDeque<u64>,
}

impl History {
    fn push(&mut self, v: u64) {
        if self.buf.len() >= HISTORY_LEN {
            self.buf.pop_front();
        }
        self.buf.push_back(v);
    }

    fn as_vec(&self) -> Vec<u64> {
        self.buf.iter().copied().collect()
    }
}

#[derive(Clone)]
#[allow(dead_code)]
struct DiskInfo {
    name: String,
    mount: String,
    used_bytes: u64,
    total_bytes: u64,
}

#[derive(Clone, Default)]
struct SystemMetrics {
    hostname: String,
    os_version: String,
    kernel_version: String,
    uptime_secs: u64,
    cpu_global_pct: f32,
    cpu_per_core_pct: Vec<f32>,
    cpu_history: History,
    mem_used_bytes: u64,
    mem_total_bytes: u64,
    swap_used_bytes: u64,
    swap_total_bytes: u64,
    net_rx_bps: u64,
    net_tx_bps: u64,
    net_rx_history: History,
    net_tx_history: History,
    disk_read_bps: u64,
    disk_write_bps: u64,
    disk_read_history: History,
    disk_write_history: History,
    disks: Vec<DiskInfo>,
    process_count: usize,
}

struct Telemetry {
    sys: RwLock<System>,
    networks: RwLock<Networks>,
    metrics: RwLock<SystemMetrics>,
}

fn bytes_to_mb(b: u64) -> u64 {
    b / 1024 / 1024
}

fn bytes_to_gb(b: u64) -> f64 {
    b as f64 / 1024.0 / 1024.0 / 1024.0
}

fn human_bytes_per_sec(bps: u64) -> String {
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

fn format_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

fn spawn_telemetry_worker(telemetry: Arc<Telemetry>, running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut prev_disk_read: u64 = 0;
        let mut prev_disk_write: u64 = 0;
        let mut prev_tick = Instant::now();
        let mut first_tick = true;
        let mut disks = Disks::new_with_refreshed_list();

        {
            let mut sys = telemetry.sys.write().unwrap();
            let mut networks = telemetry.networks.write().unwrap();
            sys.refresh_all();
            networks.refresh_list();
        }

        while running.load(Ordering::Relaxed) {
            let now = Instant::now();
            let elapsed = now.duration_since(prev_tick).as_secs_f64().max(0.001);
            prev_tick = now;

            disks.refresh();

            let (
                hostname,
                os_version,
                kernel_version,
                uptime_secs,
                cpu_global_pct,
                cpu_per_core_pct,
                mem_used_bytes,
                mem_total_bytes,
                swap_used_bytes,
                swap_total_bytes,
                process_count,
                total_read,
                total_write,
                rx_bytes,
                tx_bytes,
                disk_infos,
            ) = {
                let mut sys = telemetry.sys.write().unwrap();
                let mut networks = telemetry.networks.write().unwrap();

                sys.refresh_cpu();
                sys.refresh_memory();
                sys.refresh_processes();
                
                // Active polling for network interfaces and traffic counters
                networks.refresh();

                let mut total_read = 0u64;
                let mut total_write = 0u64;
                for proc_ in sys.processes().values() {
                    let du = proc_.disk_usage();
                    total_read += du.total_read_bytes;
                    total_write += du.total_written_bytes;
                }

                let (rx_bytes, tx_bytes) =
                    networks.iter().fold((0u64, 0u64), |(rx, tx), (_, data)| {
                        (rx + data.received(), tx + data.transmitted())
                    });

                let disk_infos: Vec<DiskInfo> = disks
                    .iter()
                    .map(|d| DiskInfo {
                        name: d.name().to_string_lossy().to_string(),
                        mount: d.mount_point().to_string_lossy().to_string(),
                        used_bytes: d.total_space().saturating_sub(d.available_space()),
                        total_bytes: d.total_space(),
                    })
                    .collect();

                (
                    System::host_name().unwrap_or_else(|| "unknown".into()),
                    System::long_os_version().unwrap_or_else(|| "unknown".into()),
                    System::kernel_version().unwrap_or_else(|| "unknown".into()),
                    System::uptime(),
                    sys.global_cpu_info().cpu_usage(),
                    sys.cpus().iter().map(|c| c.cpu_usage()).collect::<Vec<_>>(),
                    sys.used_memory(),
                    sys.total_memory(),
                    sys.used_swap(),
                    sys.total_swap(),
                    sys.processes().len(),
                    total_read,
                    total_write,
                    rx_bytes,
                    tx_bytes,
                    disk_infos,
                )
            };

            let (disk_read_bps, disk_write_bps) = if first_tick {
                (0, 0)
            } else {
                (
                    ((total_read.saturating_sub(prev_disk_read)) as f64 / elapsed) as u64,
                    ((total_write.saturating_sub(prev_disk_write)) as f64 / elapsed) as u64,
                )
            };
            prev_disk_read = total_read;
            prev_disk_write = total_write;

            let net_rx_bps = (rx_bytes as f64 / elapsed) as u64;
            let net_tx_bps = (tx_bytes as f64 / elapsed) as u64;

            {
                let mut m = telemetry.metrics.write().unwrap();
                m.hostname = hostname;
                m.os_version = os_version;
                m.kernel_version = kernel_version;
                m.uptime_secs = uptime_secs;
                m.cpu_global_pct = cpu_global_pct;
                m.cpu_per_core_pct = cpu_per_core_pct;
                m.cpu_history.push(cpu_global_pct.clamp(0.0, 100.0) as u64);
                m.mem_used_bytes = mem_used_bytes;
                m.mem_total_bytes = mem_total_bytes;
                m.swap_used_bytes = swap_used_bytes;
                m.swap_total_bytes = swap_total_bytes;
                m.net_rx_bps = net_rx_bps;
                m.net_tx_bps = net_tx_bps;
                m.net_rx_history.push(net_rx_bps / 1024);
                m.net_tx_history.push(net_tx_bps / 1024);
                m.disk_read_bps = disk_read_bps;
                m.disk_write_bps = disk_write_bps;
                m.disk_read_history.push(disk_read_bps / 1024);
                m.disk_write_history.push(disk_write_bps / 1024);
                m.disks = disk_infos;
                m.process_count = process_count;
            }

            first_tick = false;
            thread::sleep(Duration::from_millis(TICK_MS));
        }
    });
}

struct App {
    pub users: sysinfo::Users,
    telemetry: Arc<Telemetry>,
    running: Arc<AtomicBool>,
    filter: ProcessFilter,
    paused: bool,
    selected: usize,
    table_state: TableState,
    status_msg: Arc<Mutex<Option<(String, Instant)>>>,
    cached_metrics: Option<SystemMetrics>,
    cached_rows: Vec<ProcessRow>,
}

impl App {
    fn new() -> Self {
        let telemetry = Arc::new(Telemetry {
            sys: RwLock::new(System::new_all()),
            networks: RwLock::new(Networks::new_with_refreshed_list()),
            metrics: RwLock::new(SystemMetrics::default()),
        });

        Self {
            users: sysinfo::Users::new_with_refreshed_list(),
            telemetry,
            running: Arc::new(AtomicBool::new(true)),
            filter: ProcessFilter::new(),
            paused: false,
            selected: 0,
            table_state: TableState::default(),
            status_msg: Arc::new(Mutex::new(None)),
            cached_metrics: None,
            cached_rows: Vec::new(),
        }
    }

    fn flash(&self, msg: impl Into<String>) {
        *self.status_msg.lock().unwrap() = Some((msg.into(), Instant::now()));
    }

    fn current_status(&self) -> Option<String> {
        let mut guard = self.status_msg.lock().unwrap();
        if let Some((msg, at)) = guard.as_ref() {
            if at.elapsed() < Duration::from_secs(3) {
                return Some(msg.clone());
            }
            *guard = None;
        }
        None
    }

    fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    fn process_rows(&self) -> Vec<ProcessRow> {
        let sys = self.telemetry.sys.read().unwrap();
        self.filter.apply(&sys, &self.users)
    }

    fn kill_selected_process(&self, rows: &[ProcessRow]) {
        if rows.is_empty() || self.selected >= rows.len() {
            return;
        }
        let target = &rows[self.selected];
        let sys = self.telemetry.sys.read().unwrap();
        if let Some(proc_) = sys.process(target.pid.into()) {
            if proc_.kill() {
                self.flash(format!("Terminated PID {} ({})", target.pid, target.cmd));
            } else {
                self.flash(format!(
                    "Failed to kill PID {} (Permission denied)",
                    target.pid
                ));
            }
        } else {
            self.flash(format!("PID {} not found", target.pid));
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            crossterm::event::DisableMouseCapture,
            LeaveAlternateScreen
        );
        default_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    spawn_telemetry_worker(Arc::clone(&app.telemetry), Arc::clone(&app.running));

    let res = run_main_loop(&mut terminal, &mut app);

    app.stop();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        crossterm::event::DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Monitor Fault: {err:?}");
    }

    Ok(())
}

fn run_main_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    let mut proc_table_area = Rect::default();

    loop {
        let (metrics, rows) = if app.paused {
            let m = app.cached_metrics.clone().unwrap_or_default();
            let r = app.cached_rows.clone();
            (m, r)
        } else {
            let m = app.telemetry.metrics.read().unwrap().clone();
            let r = app.process_rows();
            app.cached_metrics = Some(m.clone());
            app.cached_rows = r.clone();
            (m, r)
        };

        let visible_rows = rows.len();
        if app.selected >= visible_rows && visible_rows > 0 {
            app.selected = visible_rows - 1;
        }

        terminal.draw(|f| {
            proc_table_area = draw_ui(f, app, &metrics, &rows);
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != event::KeyEventKind::Press {
                        continue;
                    }

                    if app.filter.is_active() {
                        match key.code {
                            KeyCode::Enter => {
                                app.filter.toggle_active();
                                app.selected = 0;
                            }
                            KeyCode::Esc => {
                                app.filter.clear();
                                app.selected = 0;
                            }
                            KeyCode::Backspace => app.filter.pop_char(),
                            KeyCode::Char(c) => app.filter.push_char(c),
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break
                        }
                        KeyCode::Char('/') => app.filter.toggle_active(),
                        KeyCode::Esc => {
                            app.filter.clear();
                            app.selected = 0;
                        }
                        KeyCode::Char(' ') => {
                            app.paused = !app.paused;
                            app.flash(if app.paused {
                                "Paused (Frozen)"
                            } else {
                                "Resumed"
                            });
                        }
                        KeyCode::Char('s') => {
                            app.filter.cycle_sort();
                            app.flash(format!("Sort: {}", app.filter.sort_key().label()));
                        }
                        KeyCode::Char('r') => {
                            app.filter.toggle_sort_direction();
                            let desc = app.filter.sort_desc();
                            app.flash(if desc { "Desc" } else { "Asc" });
                        }
                        KeyCode::Delete => {
                            app.kill_selected_process(&rows);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if app.selected + 1 < visible_rows {
                                app.selected += 1;
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.selected = app.selected.saturating_sub(1);
                        }
                        KeyCode::PageDown => {
                            app.selected = (app.selected + 10).min(visible_rows.saturating_sub(1));
                        }
                        KeyCode::PageUp => {
                            app.selected = app.selected.saturating_sub(10);
                        }
                        KeyCode::Home => app.selected = 0,
                        KeyCode::End => app.selected = visible_rows.saturating_sub(1),
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollDown => {
                        if app.selected + 1 < visible_rows {
                            app.selected += 1;
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        app.selected = app.selected.saturating_sub(1);
                    }
                    MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                        let mx = mouse.column;
                        let my = mouse.row;

                        if mx >= proc_table_area.x
                            && mx < proc_table_area.x + proc_table_area.width
                            && my > proc_table_area.y + 1
                            && my < proc_table_area.y + proc_table_area.height - 1
                        {
                            let clicked_row = (my - proc_table_area.y - 2) as usize;
                            let offset = app.table_state.offset();
                            let target_idx = offset + clicked_row;
                            if target_idx < visible_rows {
                                app.selected = target_idx;
                                app.flash(format!("Selected Row #{}", target_idx + 1));
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    Ok(())
}

fn draw_ui(f: &mut Frame, app: &mut App, m: &SystemMetrics, rows: &[ProcessRow]) -> Rect {
    let root_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12),
            Constraint::Min(14),
            Constraint::Length(1),
        ])
        .split(f.size());

    let bottom_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(root_chunks[1]);

    let left_stack = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(9),
            Constraint::Min(7),
        ])
        .split(bottom_split[0]);

    draw_cpu_btop(f, root_chunks[0], m);
    draw_side_metrics(f, left_stack[0], left_stack[1], left_stack[2], m);
    draw_process_matrix(f, bottom_split[1], app, rows);
    draw_status_bar(f, root_chunks[2], app);

    bottom_split[1]
}

fn draw_cpu_btop(f: &mut Frame, area: Rect, m: &SystemMetrics) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area);

    let spark_data = m.cpu_history.as_vec();
    let cpu_title = format!(
        " 1cpu │ CPU Load ({:.1}%) │ {} ({}) ",
        m.cpu_global_pct,
        m.hostname,
        format_uptime(m.uptime_secs)
    );
    let cpu_box = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green))
        .title(cpu_title);

    let inner_cpu = cpu_box.inner(split[0]);
    f.render_widget(cpu_box, split[0]);

    if inner_cpu.height > 0 {
        let main_spark = Sparkline::default()
            .data(&spark_data)
            .max(100)
            .style(Style::default().fg(Color::LightGreen));
        f.render_widget(main_spark, inner_cpu);
    }

    let core_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green))
        .title(format!(" Cores ({}) ", m.cpu_per_core_pct.len()));
    let inner = core_block.inner(split[1]);
    f.render_widget(core_block, split[1]);

    let n = m.cpu_per_core_pct.len();
    if n > 0 && inner.height > 0 {
        let cols_per_row = (inner.width as usize / 14).max(1);
        let rows_needed = n.div_ceil(cols_per_row);
        let row_constraints: Vec<Constraint> = (0..rows_needed.min(inner.height as usize))
            .map(|_| Constraint::Length(1))
            .collect();

        if !row_constraints.is_empty() {
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

                let seg_constraints: Vec<Constraint> = (start..end)
                    .map(|_| Constraint::Ratio(1, (end - start) as u32))
                    .collect();
                let segs = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(seg_constraints)
                    .split(*row_area);

                for (i, core_idx) in (start..end).enumerate() {
                    let pct = m.cpu_per_core_pct[core_idx];
                    let color = if pct > 85.0 {
                        Color::LightRed
                    } else if pct > 55.0 {
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
}

fn draw_side_metrics(
    f: &mut Frame,
    mem_area: Rect,
    disk_area: Rect,
    net_area: Rect,
    m: &SystemMetrics,
) {
    let mem_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" 2mem │ memory ");
    let mem_inner = mem_block.inner(mem_area);
    f.render_widget(mem_block, mem_area);

    let mem_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(mem_inner);

    let mem_ratio = if m.mem_total_bytes > 0 {
        m.mem_used_bytes as f64 / m.mem_total_bytes as f64
    } else {
        0.0
    };
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Yellow))
            .ratio(mem_ratio.clamp(0.0, 1.0))
            .label(format!(
                "RAM: {}/{}MB",
                bytes_to_mb(m.mem_used_bytes),
                bytes_to_mb(m.mem_total_bytes)
            )),
        mem_split[0],
    );

    let swap_ratio = if m.swap_total_bytes > 0 {
        m.swap_used_bytes as f64 / m.swap_total_bytes as f64
    } else {
        0.0
    };
    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Magenta))
            .ratio(swap_ratio.clamp(0.0, 1.0))
            .label(format!(
                "SWP: {}/{}MB",
                bytes_to_mb(m.swap_used_bytes),
                bytes_to_mb(m.swap_total_bytes)
            )),
        mem_split[1],
    );

    let disk_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Indexed(208)))
        .title(format!(
            " 3disk │ R:{} W:{} ",
            human_bytes_per_sec(m.disk_read_bps),
            human_bytes_per_sec(m.disk_write_bps)
        ));
    let disk_inner = disk_block.inner(disk_area);
    f.render_widget(disk_block, disk_area);

    let disk_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(2)])
        .split(disk_inner);

    let io_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(disk_layout[0]);

    f.render_widget(
        Sparkline::default()
            .block(Block::default().title("Read"))
            .data(&m.disk_read_history.as_vec())
            .style(Style::default().fg(Color::Indexed(208))),
        io_split[0],
    );
    f.render_widget(
        Sparkline::default()
            .block(Block::default().title("Write"))
            .data(&m.disk_write_history.as_vec())
            .style(Style::default().fg(Color::LightRed)),
        io_split[1],
    );

    if !m.disks.is_empty() && disk_layout[1].height > 0 {
        let max_disks = disk_layout[1].height as usize;
        let disk_constraints: Vec<Constraint> = (0..m.disks.len().min(max_disks))
            .map(|_| Constraint::Length(1))
            .collect();

        let d_areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(disk_constraints)
            .split(disk_layout[1]);

        for (idx, disk) in m.disks.iter().take(max_disks).enumerate() {
            let ratio = if disk.total_bytes > 0 {
                disk.used_bytes as f64 / disk.total_bytes as f64
            } else {
                0.0
            };
            let used_gb = bytes_to_gb(disk.used_bytes);
            let total_gb = bytes_to_gb(disk.total_bytes);
            let label = format!("{:<6} {:.1}/{:.1}GB", disk.mount, used_gb, total_gb);

            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(Color::Indexed(208)))
                .ratio(ratio.clamp(0.0, 1.0))
                .label(label);

            f.render_widget(gauge, d_areas[idx]);
        }
    }

    let net_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Blue))
        .title(format!(
            " net │ ↓{} ↑{} ",
            human_bytes_per_sec(m.net_rx_bps),
            human_bytes_per_sec(m.net_tx_bps)
        ));
    let net_inner = net_block.inner(net_area);
    f.render_widget(net_block, net_area);

    let net_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(net_inner);

    f.render_widget(
        Sparkline::default()
            .block(Block::default().title("RX"))
            .data(&m.net_rx_history.as_vec())
            .style(Style::default().fg(Color::Cyan)),
        net_split[0],
    );
    f.render_widget(
        Sparkline::default()
            .block(Block::default().title("TX"))
            .data(&m.net_tx_history.as_vec())
            .style(Style::default().fg(Color::LightBlue)),
        net_split[1],
    );
}

fn draw_process_matrix(f: &mut Frame, area: Rect, app: &mut App, rows: &[ProcessRow]) {
    let title = if app.filter.is_active() {
        format!(" 4proc │ filter: {}_ ", app.filter.get_query())
    } else if app.filter.has_query() {
        format!(
            " 4proc │ Filter: \"{}\" ({} matches, sort: {}) ",
            app.filter.get_query(),
            rows.len(),
            app.filter.sort_key().label()
        )
    } else {
        format!(
            " 4proc │ Process Matrix ({} total, sort: {}) ",
            rows.len(),
            app.filter.sort_key().label()
        )
    };

    let header = Row::new(vec![
        "PID", "USER", "CPU%", "MEM%", "MEM(MB)", "ST", "COMMAND",
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Cyan),
    );

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|p| {
            let mut style = Style::default();
            if p.cpu_pct > 40.0 {
                style = style.fg(Color::LightRed);
            } else if p.cpu_pct > 15.0 {
                style = style.fg(Color::Yellow);
            }
            Row::new(vec![
                Cell::from(p.pid.to_string()),
                Cell::from(p.user.clone()),
                Cell::from(format!("{:.1}", p.cpu_pct)),
                Cell::from(format!("{:.1}", p.mem_pct)),
                Cell::from(p.mem_mb.to_string()),
                Cell::from(p.state.to_string()),
                Cell::from(p.cmd.clone()),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(7),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(9),
        Constraint::Length(3),
        Constraint::Min(20),
    ];

    let table = Table::new(table_rows, widths)
        .header(header)
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(236))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯ ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Magenta))
                .title(title),
        );

    app.table_state.select(if rows.is_empty() {
        None
    } else {
        Some(app.selected)
    });
    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let text = if let Some(msg) = app.current_status() {
        msg
    } else {
        "/ filter  s sort  r direction  k kill  space pause  ↑↓/jk navigate  q quit".to_string()
    };
    let bar = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(bar, area);
}