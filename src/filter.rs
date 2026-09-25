use sysinfo::{ProcessStatus, System, Users};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    Cpu,
    Mem,
    Pid,
    Name,
}

impl SortKey {
    pub fn label(&self) -> &'static str {
        match self {
            SortKey::Cpu => "CPU%",
            SortKey::Mem => "MEM%",
            SortKey::Pid => "PID",
            SortKey::Name => "NAME",
        }
    }
}

#[derive(Clone)]
pub struct ProcessRow {
    pub pid: usize,
    pub user: String,
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub mem_mb: u64,
    pub state: char,
    pub cmd: String,
}

#[derive(Debug, Clone)]
pub struct ProcessFilter {
    query: String,
    active: bool,
    sort_key: SortKey,
    sort_desc: bool,
}

impl Default for ProcessFilter {
    fn default() -> Self {
        Self {
            query: String::new(),
            active: false,
            sort_key: SortKey::Cpu,
            sort_desc: true,
        }
    }
}

impl ProcessFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn toggle_active(&mut self) {
        self.active = !self.active;
    }

    pub fn has_query(&self) -> bool {
        !self.query.is_empty()
    }

    pub fn get_query(&self) -> &str {
        &self.query
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn pop_char(&mut self) {
        self.query.pop();
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.active = false;
    }

    pub fn sort_key(&self) -> SortKey {
        self.sort_key
    }

    pub fn sort_desc(&self) -> bool {
        self.sort_desc
    }

    pub fn cycle_sort(&mut self) {
        self.sort_key = match self.sort_key {
            SortKey::Cpu => SortKey::Mem,
            SortKey::Mem => SortKey::Pid,
            SortKey::Pid => SortKey::Name,
            SortKey::Name => SortKey::Cpu,
        };
    }

    pub fn toggle_sort_direction(&mut self) {
        self.sort_desc = !self.sort_desc;
    }

    /// Applies process filtering and sorting.
    /// Expects a reference to a cached `Users` instance to prevent heavy file I/O operations per render frame.
    pub fn apply(&self, sys: &System, users: &Users) -> Vec<ProcessRow> {
        let total_mem = sys.total_memory().max(1) as f32;

        let mut rows: Vec<ProcessRow> = sys
            .processes()
            .iter()
            .map(|(&pid, p)| {
                let user = p
                    .user_id()
                    .and_then(|uid| users.get_user_by_id(uid))
                    .map(|u| u.name().to_string())
                    .unwrap_or_else(|| "unknown".into());

                let cmd = {
                    let c = p.cmd();
                    if c.is_empty() {
                        p.name().to_string()
                    } else {
                        c.iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    }
                };

                let state_char = match p.status() {
                    ProcessStatus::Run => 'R',
                    ProcessStatus::Sleep => 'S',
                    ProcessStatus::Idle => 'I',
                    ProcessStatus::Zombie => 'Z',
                    ProcessStatus::Stop => 'T',
                    _ => '?',
                };

                let memory = p.memory();

                ProcessRow {
                    pid: pid.as_u32() as usize,
                    user,
                    cpu_pct: p.cpu_usage(),
                    mem_pct: (memory as f32 / total_mem) * 100.0,
                    mem_mb: memory / (1024 * 1024),
                    state: state_char,
                    cmd,
                }
            })
            .collect();

        if !self.query.is_empty() {
            let q = self.query.to_lowercase();
            rows.retain(|r| {
                r.cmd.to_lowercase().contains(&q)
                    || r.user.to_lowercase().contains(&q)
                    || r.pid.to_string().contains(&q)
            });
        }

        rows.sort_by(|a, b| {
            let ord = match self.sort_key {
                SortKey::Cpu => a
                    .cpu_pct
                    .partial_cmp(&b.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortKey::Mem => a
                    .mem_pct
                    .partial_cmp(&b.mem_pct)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortKey::Pid => a.pid.cmp(&b.pid),
                SortKey::Name => a.cmd.to_lowercase().cmp(&b.cmd.to_lowercase()),
            };

            if self.sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });

        rows
    }
}


