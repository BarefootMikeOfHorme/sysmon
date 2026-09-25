pub enum SidebarSection {
    System,
    Performance,
    AiCluster,
    DevTools,
    Sandboxes,
    DebugTab(usize), // Dynamic sub-tabs for detached files/processes
}

pub struct DebugSubTab {
    pub id: usize,
    pub title: String,         // e.g., "local_inference.py"
    pub target_pid: u32,
    pub icon: String,          // e.g., "🐞"
}