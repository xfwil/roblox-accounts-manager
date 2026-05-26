use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::RobloxClient;

/// A game server instance
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameServer {
    pub id: String,
    pub max_players: u32,
    pub playing: u32,
    pub player_tokens: Vec<String>,
    pub fps: f64,
    pub ping: Option<u32>,
}

/// Response from the game servers API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameServersResponse {
    data: Vec<GameServer>,
    next_page_cursor: Option<String>,
}

/// Game/place details
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetails {
    pub place_id: u64,
    pub name: String,
    pub description: Option<String>,
    pub builder: Option<String>,
    pub builder_id: Option<u64>,
    pub max_players: Option<u32>,
    pub playing: Option<u64>,
    pub visits: Option<u64>,
    pub favorites_count: Option<u64>,
    pub image_token: Option<String>,
}

/// Response from games multiget
#[derive(Debug, Deserialize)]
struct GamesResponse {
    data: Vec<GameDetailsRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameDetailsRaw {
    id: u64,
    root_place_id: Option<u64>,
    name: String,
    description: Option<String>,
    creator: Option<CreatorInfo>,
    max_players: Option<u32>,
    playing: Option<u64>,
    visits: Option<u64>,
    #[serde(rename = "favoritedCount")]
    favorited_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatorInfo {
    id: u64,
    name: String,
}

/// Universe ID lookup response
#[derive(Debug, Deserialize)]
struct UniverseIdResponse {
    #[serde(rename = "universeId")]
    universe_id: u64,
}

impl RobloxClient {
    /// Get the universe ID for a place
    pub async fn get_universe_id(&self, place_id: u64) -> AppResult<u64> {
        let url = format!(
            "https://apis.roblox.com/universes/v1/places/{}/universe",
            place_id
        );

        let resp = self.client().get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi(format!(
                "Failed to get universe ID for place {}: {}",
                place_id,
                resp.status()
            )));
        }

        let data: UniverseIdResponse = resp.json().await?;
        Ok(data.universe_id)
    }

    /// Get game details by universe IDs
    pub async fn get_game_details(&self, universe_ids: &[u64]) -> AppResult<Vec<GameDetails>> {
        if universe_ids.is_empty() {
            return Ok(vec![]);
        }

        let ids_str: Vec<String> = universe_ids.iter().map(|id| id.to_string()).collect();
        let url = format!(
            "https://games.roblox.com/v1/games?universeIds={}",
            ids_str.join(",")
        );

        let resp = self.client().get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi(format!(
                "Failed to get game details: {}",
                resp.status()
            )));
        }

        let data: GamesResponse = resp.json().await?;
        Ok(data
            .data
            .into_iter()
            .map(|g| GameDetails {
                place_id: g.root_place_id.unwrap_or(g.id),
                name: g.name,
                description: g.description,
                builder: g.creator.as_ref().map(|c| c.name.clone()),
                builder_id: g.creator.as_ref().map(|c| c.id),
                max_players: g.max_players,
                playing: g.playing,
                visits: g.visits,
                favorites_count: g.favorited_count,
                image_token: None,
            })
            .collect())
    }

    /// Get servers for a place
    pub async fn get_servers(
        &self,
        place_id: u64,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> AppResult<(Vec<GameServer>, Option<String>)> {
        let limit = limit.unwrap_or(10).min(100);
        let mut url = format!(
            "https://games.roblox.com/v1/games/{}/servers/Public?sortOrder=Asc&excludeFullGames=false&limit={}",
            place_id, limit
        );

        if let Some(cursor) = cursor {
            url.push_str(&format!("&cursor={}", cursor));
        }

        let resp = self.client().get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi(format!(
                "Failed to get servers for place {}: {}",
                place_id,
                resp.status()
            )));
        }

        let data: GameServersResponse = resp.json().await?;
        Ok((data.data, data.next_page_cursor))
    }
}
