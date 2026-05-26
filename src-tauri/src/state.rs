use std::sync::{Arc, Mutex};

use crate::roblox::RobloxClient;
use crate::storage::AccountStore;

/// Application state shared across Tauri commands and the Web API server
pub struct AppState {
    pub store: Arc<Mutex<AccountStore>>,
    pub roblox: RobloxClient,
}
