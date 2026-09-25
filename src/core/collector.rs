use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System, Users};

/// Aggregated telemetry snapshot for UI gauges and widgets.
#[derive(Debug, Clone, Default)]
pub struct SystemMetrics {
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,
    pub global_cpu_usage: f32,
    pub cpus_usage: Vec<f32>,
    pub uptime: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Shared system state managed by the collector worker thread.
pub struct CollectorState {
    pub sys: System,
    pub users: Users,
    pub disks: Disks,
    pub networks: Networks,
    pub components: Components,
    pub metrics: SystemMetrics,
}

impl CollectorState {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        Self {
            sys,
            users: Users::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            metrics: SystemMetrics::default(),
        }
    }

    /// Primary refresh routine called periodically by the worker thread.
    pub fn refresh(&mut self) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        self.networks.refresh(true);
        self.disks.refresh(true);
        self.components.refresh(true);

        let (mut total_rx, mut total_tx) = (0, 0);
        for (_interface_name, network) in &self.networks {
            total_rx += network.received();
            total_tx += network.transmitted();
        }

        let cpus_usage = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();

        self.metrics = SystemMetrics {
            total_memory: self.sys.total_memory(),
            used_memory: self.sys.used_memory(),
            total_swap: self.sys.total_swap(),
            used_swap: self.sys.used_swap(),
            global_cpu_usage: self.sys.global_cpu_info().cpu_usage(),
            cpus_usage,
            uptime: System::uptime(),
            rx_bytes: total_rx,
            tx_bytes: total_tx,
        };
    }
}

impl Default for CollectorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Controller handle for the background telemetry collector thread.
pub struct TelemetryCollector {
    state: Arc<Mutex<CollectorState>>,
    running: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
}

impl TelemetryCollector {
    pub fn spawn(refresh_interval: Duration) -> Self {
        let state = Arc::new(Mutex::new(CollectorState::new()));
        let running = Arc::new(AtomicBool::new(true));

        let state_clone = Arc::clone(&state);
        let running_clone = Arc::clone(&running);

        let worker_handle = thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                if let Ok(mut guard) = state_clone.lock() {
                    guard.refresh();
                }
                thread::sleep(refresh_interval);
            }
        });

        Self {
            state,
            running,
            worker_handle: Some(worker_handle),
        }
    }

    pub fn state(&self) -> Arc<Mutex<CollectorState>> {
        Arc::clone(&self.state)
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for TelemetryCollector {
    fn drop(&mut self) {
        self.stop();
    }
}