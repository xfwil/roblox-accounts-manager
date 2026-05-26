use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sysinfo::System;

/// Tracks running Roblox processes and their associated accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedProcess {
    pub pid: u32,
    pub account_id: String,
    pub place_id: u64,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// Process status for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStatus {
    pub account_id: String,
    pub pid: u32,
    pub place_id: u64,
    pub is_running: bool,
    pub started_at: String,
    pub uptime_seconds: i64,
}

/// Watcher state — tracks running Roblox instances
pub struct WatcherState {
    /// Map of PID -> tracked process
    tracked: HashMap<u32, TrackedProcess>,
}

impl WatcherState {
    pub fn new() -> Self {
        Self {
            tracked: HashMap::new(),
        }
    }

    /// Track a new process
    pub fn track(&mut self, pid: u32, account_id: String, place_id: u64) {
        self.tracked.insert(
            pid,
            TrackedProcess {
                pid,
                account_id,
                place_id,
                started_at: chrono::Utc::now(),
            },
        );
    }

    /// Remove a tracked process
    pub fn untrack(&mut self, pid: u32) {
        self.tracked.remove(&pid);
    }

    /// Get all tracked processes with their current status
    pub fn get_status(&self) -> Vec<ProcessStatus> {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let now = chrono::Utc::now();

        self.tracked
            .values()
            .map(|tp| {
                let is_running = sys
                    .process(sysinfo::Pid::from_u32(tp.pid))
                    .is_some();

                ProcessStatus {
                    account_id: tp.account_id.clone(),
                    pid: tp.pid,
                    place_id: tp.place_id,
                    is_running,
                    started_at: tp.started_at.to_rfc3339(),
                    uptime_seconds: (now - tp.started_at).num_seconds(),
                }
            })
            .collect()
    }

    /// Clean up dead processes, returns list of dead process info
    pub fn cleanup_dead(&mut self) -> Vec<TrackedProcess> {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let dead_pids: Vec<u32> = self
            .tracked
            .keys()
            .filter(|pid| {
                sys.process(sysinfo::Pid::from_u32(**pid)).is_none()
            })
            .copied()
            .collect();

        let mut dead = Vec::new();
        for pid in dead_pids {
            if let Some(tp) = self.tracked.remove(&pid) {
                dead.push(tp);
            }
        }

        dead
    }

    /// Get count of running instances
    pub fn running_count(&self) -> usize {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        self.tracked
            .keys()
            .filter(|pid| {
                sys.process(sysinfo::Pid::from_u32(**pid)).is_some()
            })
            .count()
    }

    /// Find all Roblox processes (even untracked ones)
    pub fn find_roblox_processes() -> Vec<(u32, String)> {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        sys.processes()
            .iter()
            .filter(|(_, p)| {
                let name = p.name().to_string_lossy().to_lowercase();
                name.contains("robloxplayerbeta") || name.contains("robloxplayer")
            })
            .map(|(pid, p)| (pid.as_u32(), p.name().to_string_lossy().to_string()))
            .collect()
    }
}

/// Shared watcher state
pub type SharedWatcher = Arc<Mutex<WatcherState>>;

pub fn new_watcher() -> SharedWatcher {
    Arc::new(Mutex::new(WatcherState::new()))
}
