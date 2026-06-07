use std::sync::{Arc, Mutex};

use crate::roblox::RobloxClient;
use crate::storage::AccountStore;

/// Throttle state for join operations to avoid rapid-fire requests
/// that trigger CAPTCHA/rate limiting from Roblox.
pub struct JoinThrottle {
    /// Timestamp of the last join request (epoch millis)
    pub last_join: std::time::Instant,
    /// Minimum delay between joins in milliseconds (default: 3000ms)
    pub min_delay_ms: u64,
}

impl JoinThrottle {
    pub fn new() -> Self {
        Self {
            // Start far in the past so first join is instant
            last_join: std::time::Instant::now() - std::time::Duration::from_secs(60),
            min_delay_ms: 3000,
        }
    }
}

/// Application state shared across Tauri commands and the Web API server
pub struct AppState {
    pub store: Arc<Mutex<AccountStore>>,
    pub roblox: RobloxClient,
    pub join_throttle: Arc<tokio::sync::Mutex<JoinThrottle>>,
}
