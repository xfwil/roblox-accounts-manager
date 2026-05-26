use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::RobloxClient;

/// User presence type from Roblox API
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresenceType {
    Offline = 0,
    Online = 1,
    InGame = 2,
    InStudio = 3,
}

impl From<u8> for PresenceType {
    fn from(val: u8) -> Self {
        match val {
            1 => PresenceType::Online,
            2 => PresenceType::InGame,
            3 => PresenceType::InStudio,
            _ => PresenceType::Offline,
        }
    }
}

/// Presence info for a single user
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPresence {
    pub user_id: u64,
    pub presence_type: u8,
    pub last_location: String,
    pub place_id: Option<u64>,
    pub game_id: Option<String>,
    pub universe_id: Option<u64>,
    pub last_online: Option<String>,
}

/// Raw response from Roblox presence API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresenceResponse {
    user_presences: Vec<RawPresence>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPresence {
    user_id: u64,
    user_presence_type: u8,
    last_location: Option<String>,
    place_id: Option<u64>,
    game_id: Option<String>,
    universe_id: Option<u64>,
    last_online: Option<String>,
}

impl RobloxClient {
    /// Get presence for multiple users (requires authentication)
    /// POST https://presence.roblox.com/v1/presence/users
    pub async fn get_user_presences(
        &self,
        cookie: &str,
        user_ids: &[u64],
    ) -> AppResult<Vec<UserPresence>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        // API limit: max 50 user IDs per request
        let mut all_presences = Vec::new();

        for chunk in user_ids.chunks(50) {
            let resp = self
                .client()
                .post("https://presence.roblox.com/v1/presence/users")
                .headers(Self::auth_headers(cookie))
                .json(&serde_json::json!({ "userIds": chunk }))
                .send()
                .await?;

            if !resp.status().is_success() {
                return Err(AppError::RobloxApi(format!(
                    "Failed to get user presences: {}",
                    resp.status()
                )));
            }

            let data: PresenceResponse = resp.json().await?;

            for raw in data.user_presences {
                all_presences.push(UserPresence {
                    user_id: raw.user_id,
                    presence_type: raw.user_presence_type,
                    last_location: raw.last_location.unwrap_or_default(),
                    place_id: raw.place_id,
                    game_id: raw.game_id,
                    universe_id: raw.universe_id,
                    last_online: raw.last_online,
                });
            }
        }

        Ok(all_presences)
    }
}
