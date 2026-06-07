use tauri::State;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::roblox::{GameDetails, GameServer, LaunchOptions, RobloxClient};
use crate::state::AppState;

/// Launch Roblox for an account with the given place ID
#[tauri::command]
pub async fn join_game(
    account_id: String,
    place_id: u64,
    job_id: Option<String>,
    follow_user_id: Option<u64>,
    link_code: Option<String>,
    launch_args: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<u32> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    // Throttle: wait if we joined too recently to avoid CAPTCHA/rate-limit
    {
        let mut throttle = state.join_throttle.lock().await;
        let elapsed = throttle.last_join.elapsed();
        let min_delay = std::time::Duration::from_millis(throttle.min_delay_ms);
        if elapsed < min_delay {
            let wait = min_delay - elapsed;
            log::info!("[{}] Throttling join: waiting {}ms", account_id, wait.as_millis());
            tokio::time::sleep(wait).await;
        }
        throttle.last_join = std::time::Instant::now();
    }

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    // If we have a link_code, resolve the access_code (UUID) and numeric linkCode.
    // PlaceLauncher REQUIRES a valid accessCode — launching without it gives Error 524.
    let (access_code, numeric_link_code) = if let Some(ref lc) = link_code {
        log::info!("[{}] Resolving access code for place_id={}, link_code={}", account_id, place_id, lc);

        match state.roblox.resolve_access_code(&cookie, place_id, lc).await {
            Ok((ac, nlc)) => {
                log::info!("[{}] Got access_code: {}, numeric_link_code: {:?}", account_id, ac, nlc);
                (Some(ac), nlc)
            }
            Err(e) => {
                log::error!("[{}] Failed to resolve access code: {}", account_id, e);
                return Err(AppError::RobloxApi(format!(
                    "Could not resolve private server access code: {}. Make sure the link is valid and not expired.", e
                )));
            }
        }
    } else {
        (None, None)
    };

    // Get auth ticket
    let auth_ticket = state.roblox.get_auth_ticket(&cookie).await?;

    // Use the numeric linkCode for PlaceLauncher (not the share code).
    // PlaceLauncher expects linkCode to be a numeric privateServerLinkCode.
    let effective_link_code = numeric_link_code.or(
        // If the original link_code is numeric, use it directly
        link_code.filter(|lc| lc.chars().all(|c| c.is_ascii_digit()) && !lc.is_empty())
    );

    let options = LaunchOptions {
        place_id,
        job_id,
        follow_user_id,
        link_code: effective_link_code,
        access_code,
        launch_args,
    };

    let browser_tracker_id = rand::random::<u64>() % 1_000_000_000;

    // Update last_used timestamp
    {
        let mut store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        let mut account = store.get_account(&uuid)?.clone();
        account.last_used = Some(chrono::Utc::now());
        store.update_account(account)?;
    }

    let pid = RobloxClient::launch_roblox(&auth_ticket, &options, browser_tracker_id)?;
    Ok(pid)
}

/// Get servers for a place
#[tauri::command]
pub async fn get_servers(
    place_id: u64,
    cursor: Option<String>,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> AppResult<(Vec<GameServer>, Option<String>)> {
    state
        .roblox
        .get_servers(place_id, cursor.as_deref(), limit)
        .await
}

/// Get game details by place ID
#[tauri::command]
pub async fn get_game_details(
    place_id: u64,
    state: State<'_, AppState>,
) -> AppResult<Vec<GameDetails>> {
    let universe_id = state.roblox.get_universe_id(place_id).await?;
    state.roblox.get_game_details(&[universe_id]).await
}

/// Pre-resolve a private server access code using any account's cookie.
/// Call this once before joining multiple accounts to the same PS.
#[tauri::command]
pub async fn resolve_private_server(
    account_id: String,
    place_id: u64,
    link_code: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    let (access_code, _) = state.roblox.resolve_access_code(&cookie, place_id, &link_code).await?;
    Ok(access_code)
}

/// Get Roblox installation path
#[tauri::command]
pub async fn get_roblox_path() -> AppResult<String> {
    let path = RobloxClient::get_roblox_path()?;
    Ok(path.to_string_lossy().to_string())
}

/// Resolve a Roblox share link (https://www.roblox.com/share?code=...&type=Server)
/// to get the actual place ID and link code.
/// Returns (place_id, link_code)
#[tauri::command]
pub async fn resolve_share_link(
    share_url: String,
    state: State<'_, AppState>,
) -> AppResult<(u64, String)> {
    // Follow the redirect to get the actual game URL
    // Roblox share links redirect to: /games/PLACEID/...?privateServerLinkCode=CODE
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::Http(e.to_string()))?;

    let resp = client
        .get(&share_url)
        .send()
        .await?;

    // Check for redirect (301/302/303)
    if resp.status().is_redirection() {
        if let Some(location) = resp.headers().get("location") {
            let location_str = location.to_str().unwrap_or("");
            return parse_game_url(location_str);
        }
    }

    // If no redirect, try to parse the response body for meta refresh or JS redirect
    let body = resp.text().await?;

    // Look for og:url or canonical URL in HTML
    // Pattern: <meta property="og:url" content="https://www.roblox.com/games/PLACEID/...?privateServerLinkCode=CODE" />
    if let Some(start) = body.find("privateServerLinkCode=") {
        // Try to extract from the full URL in the page
        let before = &body[..start];
        if let Some(games_pos) = before.rfind("/games/") {
            let after_games = &body[games_pos + 7..];
            if let Some(slash_pos) = after_games.find('/') {
                let place_id_str = &after_games[..slash_pos];
                if let Ok(place_id) = place_id_str.parse::<u64>() {
                    let code_start = start + "privateServerLinkCode=".len();
                    let code_end = body[code_start..].find(|c: char| c == '"' || c == '&' || c == '\'' || c.is_whitespace())
                        .map(|i| code_start + i)
                        .unwrap_or(body.len());
                    let code = body[code_start..code_end].to_string();
                    if !code.is_empty() {
                        return Ok((place_id, code));
                    }
                }
            }
        }
    }

    // Try the share API endpoint directly
    // Extract code from the share URL
    if let Ok(url) = reqwest::Url::parse(&share_url) {
        if let Some(code) = url.query_pairs().find(|(k, _)| k == "code").map(|(_, v)| v.to_string()) {
            // Try the Roblox share resolution API
            let api_resp = state.roblox.client()
                .get(format!("https://apis.roblox.com/sharelinks/v1/resolve-link?code={}&type=Server", code))
                .send()
                .await;

            if let Ok(resp) = api_resp {
                if resp.status().is_success() {
                    #[derive(serde::Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct ShareLinkResponse {
                        place_id: Option<u64>,
                        link_code: Option<String>,
                        private_server_link_code: Option<String>,
                    }

                    if let Ok(data) = resp.json::<ShareLinkResponse>().await {
                        if let Some(place_id) = data.place_id {
                            let link = data.private_server_link_code
                                .or(data.link_code)
                                .unwrap_or(code);
                            return Ok((place_id, link));
                        }
                    }
                }
            }
        }
    }

    Err(AppError::Other("Could not resolve share link. Make sure the URL is a valid Roblox private server link.".into()))
}

/// Parse a Roblox game URL to extract place_id and link_code
fn parse_game_url(url: &str) -> AppResult<(u64, String)> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| AppError::Other(format!("Invalid URL: {}", url)))?;

    // Extract place ID from /games/PLACEID/...
    let path = parsed.path();
    let place_id = path
        .split('/')
        .find_map(|segment| segment.parse::<u64>().ok())
        .ok_or_else(|| AppError::Other("Could not find Place ID in URL".into()))?;

    // Extract privateServerLinkCode from query
    let link_code = parsed
        .query_pairs()
        .find(|(k, _)| k == "privateServerLinkCode")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| AppError::Other("Could not find privateServerLinkCode in URL".into()))?;

    Ok((place_id, link_code))
}

/// Check and auto-fix private server invite privacy settings.
/// If the account's privacy doesn't allow PS invites, set it to "AllUsers".
/// Same behavior as original RAM C# (Account.cs lines 583-609).
async fn ensure_ps_invite_privacy(
    roblox: &RobloxClient,
    cookie: &str,
) -> AppResult<()> {
    let cookie_header = format!(
        ".ROBLOSECURITY={}",
        cookie.trim_start_matches(".ROBLOSECURITY=")
    );
    let csrf = roblox.get_csrf_token(cookie).await?;

    // Check current privacy setting
    let resp = roblox.client()
        .get("https://www.roblox.com/account/settings/private-server-invite-privacy")
        .header("Cookie", &cookie_header)
        .header("x-csrf-token", &csrf)
        .header("Referer", "https://www.roblox.com/my/account")
        .send()
        .await?;

    let body = resp.text().await.unwrap_or_default();

    // If already set to AllUsers, nothing to do
    if body.contains("\"AllUsers\"") {
        return Ok(());
    }

    log::info!("Account PS invite privacy is not AllUsers, updating...");

    // Set to AllUsers
    let csrf = roblox.get_csrf_token(cookie).await?;
    let resp = roblox.client()
        .post("https://www.roblox.com/account/settings/private-server-invite-privacy")
        .header("Cookie", &cookie_header)
        .header("x-csrf-token", &csrf)
        .header("Referer", "https://www.roblox.com/my/account")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("PrivateServerInvitePrivacy=AllUsers")
        .send()
        .await?;

    if resp.status().is_success() {
        log::info!("Updated PS invite privacy to AllUsers");
    } else {
        log::warn!("Failed to update PS invite privacy: {}", resp.status());
    }

    Ok(())
}
