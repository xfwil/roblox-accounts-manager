use tauri::State;

use crate::error::{AppError, AppResult};
use crate::watcher::{ProcessStatus, SharedWatcher};

/// Track a launched process
#[tauri::command]
pub async fn track_process(
    pid: u32,
    account_id: String,
    place_id: u64,
    watcher: State<'_, SharedWatcher>,
) -> AppResult<()> {
    let mut w = watcher.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    w.track(pid, account_id, place_id);
    Ok(())
}

/// Untrack a process
#[tauri::command]
pub async fn untrack_process(
    pid: u32,
    watcher: State<'_, SharedWatcher>,
) -> AppResult<()> {
    let mut w = watcher.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    w.untrack(pid);
    Ok(())
}

/// Get status of all tracked processes
#[tauri::command]
pub async fn get_process_status(
    watcher: State<'_, SharedWatcher>,
) -> AppResult<Vec<ProcessStatus>> {
    let w = watcher.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    Ok(w.get_status())
}

/// Clean up dead processes
#[tauri::command]
pub async fn cleanup_dead_processes(
    watcher: State<'_, SharedWatcher>,
) -> AppResult<usize> {
    let mut w = watcher.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    let dead = w.cleanup_dead();
    Ok(dead.len())
}

/// Get count of running Roblox instances
#[tauri::command]
pub async fn get_running_count(
    watcher: State<'_, SharedWatcher>,
) -> AppResult<usize> {
    let w = watcher.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    Ok(w.running_count())
}

/// Find all Roblox processes (even untracked)
#[tauri::command]
pub async fn find_roblox_processes() -> AppResult<Vec<(u32, String)>> {
    Ok(crate::watcher::WatcherState::find_roblox_processes())
}
