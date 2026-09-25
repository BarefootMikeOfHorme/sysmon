use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

pub struct DebugStreamSession {
    pub target_pid: u32,
    pub process_name: String,
    // Live rolling window of stdout/stderr logs from the app/compiler
    pub logs: Arc<Mutex<VecDeque<String>>>,
    // Scoped hardware telemetry tied *specifically* to this process and its children
    pub telemetry_metrics: ProcessTelemetrySnapshot,
}

#[derive(Default, Clone)]
pub struct ProcessTelemetrySnapshot {
    pub cpu_usage_pct: f32,
    pub memory_rss_mb: u64,
    pub vram_allocated_mb: u64,
    pub thread_count: usize,
    pub thermal_status_warning: bool, // e.g., if core temps spike > 80°C
}