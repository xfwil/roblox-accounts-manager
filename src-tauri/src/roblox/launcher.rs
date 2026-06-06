use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use crate::error::{AppError, AppResult};

use super::RobloxClient;

/// Cache for resolved access codes: (place_id, link_code) → (access_code, numeric_link_code)
/// Prevents redundant API calls when multiple accounts join the same PS.
static ACCESS_CODE_CACHE: std::sync::LazyLock<Mutex<HashMap<(u64, String), (String, Option<String>)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Roblox game launch options
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LaunchOptions {
    /// Place ID to join
    pub place_id: u64,
    /// Specific server job ID (optional)
    pub job_id: Option<String>,
    /// Whether to follow a specific user
    pub follow_user_id: Option<u64>,
    /// Private server link code (numeric string from URL param)
    pub link_code: Option<String>,
    /// Private server access code (UUID resolved from API)
    pub access_code: Option<String>,
    /// Launch arguments
    pub launch_args: Option<String>,
}

impl RobloxClient {
    /// Get an authentication ticket for launching Roblox.
    /// Handles CSRF 403 retry: if the first request returns 403 with a fresh x-csrf-token,
    /// we retry with that token (same pattern as Roblox's own launcher).
    pub async fn get_auth_ticket(&self, cookie: &str) -> AppResult<String> {
        let cookie_header = format!(
            ".ROBLOSECURITY={}",
            cookie.trim_start_matches(".ROBLOSECURITY=")
        );

        // First, get a CSRF token
        let mut csrf = self.get_csrf_token(cookie).await?;

        // Try up to 3 times (CSRF can expire between fetch and use)
        for attempt in 0..3 {
            let resp = self
                .client()
                .post("https://auth.roblox.com/v1/authentication-ticket/")
                .header("Cookie", &cookie_header)
                .header("x-csrf-token", &csrf)
                .header("Content-Type", "application/json")
                .header("Referer", "https://www.roblox.com")
                .header("Origin", "https://www.roblox.com")
                .body("{}")
                .send()
                .await?;

            let status = resp.status();

            // Success — extract ticket from response header
            if status.is_success() {
                return resp
                    .headers()
                    .get("rbx-authentication-ticket")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                    .ok_or_else(|| AppError::RobloxApi("No auth ticket in response".into()));
            }

            // 403 with fresh CSRF token — retry with the new token
            if status.as_u16() == 403 {
                if let Some(new_csrf) = resp.headers().get("x-csrf-token").and_then(|v| v.to_str().ok()) {
                    log::info!("Auth ticket got 403 with new CSRF on attempt {}, retrying", attempt + 1);
                    csrf = new_csrf.to_string();
                    continue;
                }
            }

            // Other errors — fail immediately
            return Err(AppError::RobloxApi(format!(
                "Failed to get auth ticket: {} (attempt {})",
                status, attempt + 1
            )));
        }

        Err(AppError::RobloxApi(
            "Failed to get auth ticket after 3 CSRF retries".into(),
        ))
    }

    /// Get the Roblox Player installation path
    pub fn get_roblox_path() -> AppResult<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command as Cmd;

            let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();

            // Helper: scan a Versions directory for the most recently modified RobloxPlayerBeta.exe
            let scan_versions_dir = |versions_dir: &PathBuf| -> Option<PathBuf> {
                if !versions_dir.exists() {
                    return None;
                }
                let entries = std::fs::read_dir(versions_dir).ok()?;
                let mut latest: Option<PathBuf> = None;
                let mut latest_time = std::time::SystemTime::UNIX_EPOCH;

                for entry in entries.flatten() {
                    let path = entry.path();
                    let player = path.join("RobloxPlayerBeta.exe");
                    if player.exists() {
                        if let Ok(meta) = player.metadata() {
                            if let Ok(modified) = meta.modified() {
                                if modified > latest_time {
                                    latest_time = modified;
                                    latest = Some(player);
                                }
                            }
                        }
                    }
                }
                latest
            };

            // 1. Check standard Roblox install
            let roblox_versions = PathBuf::from(&local_app_data).join("Roblox").join("Versions");
            if let Some(path) = scan_versions_dir(&roblox_versions) {
                return Ok(path);
            }

            // 2. Check Bloxstrap/Fishstrap/Froststrap managed installs
            for launcher in &["Bloxstrap", "Fishstrap", "Froststrap"] {
                let versions_dir = PathBuf::from(&local_app_data).join(launcher).join("Versions");
                if let Some(path) = scan_versions_dir(&versions_dir) {
                    return Ok(path);
                }
            }

            // 3. Check Program Files
            let program_files = std::env::var("ProgramFiles(x86)")
                .or_else(|_| std::env::var("ProgramFiles"))
                .unwrap_or_default();
            let pf_versions = PathBuf::from(&program_files).join("Roblox").join("Versions");
            if let Some(path) = scan_versions_dir(&pf_versions) {
                return Ok(path);
            }

            // 4. Check registry protocol handler for roblox-player
            //    Format: "C:\path\to\launcher.exe" -player "%1"
            //    If it's a strap launcher, look for Versions dir next to it
            let reg_output = Cmd::new("reg")
                .args([
                    "query",
                    r"HKCU\Software\Classes\roblox-player\shell\open\command",
                    "/ve",
                ])
                .output();

            if let Ok(output) = reg_output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("REG_SZ") {
                        if let Some(val) = line.split("REG_SZ").nth(1) {
                            let val = val.trim().trim_matches('"');
                            // Extract the exe path (before any arguments)
                            let exe_path = if val.starts_with('"') {
                                val.trim_start_matches('"').split('"').next().unwrap_or("")
                            } else {
                                val.split_whitespace().next().unwrap_or("")
                            };
                            let exe = PathBuf::from(exe_path);
                            if exe.exists() {
                                // Check for Versions dir next to the launcher exe
                                if let Some(parent) = exe.parent() {
                                    let versions = parent.join("Versions");
                                    if let Some(path) = scan_versions_dir(&versions) {
                                        return Ok(path);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Err(AppError::Other(
                "Could not find Roblox installation. Please make sure Roblox is installed.".into(),
            ))
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err(AppError::Other(
                "Roblox launching is only supported on Windows".into(),
            ))
        }
    }

    /// Launch Roblox with the given auth ticket and options.
    pub fn launch_roblox(
        auth_ticket: &str,
        options: &LaunchOptions,
        browser_tracker_id: u64,
    ) -> AppResult<u32> {
        let timestamp = chrono::Utc::now().timestamp_millis();

        // Build the PlaceLauncher URL
        let launcher_url = if let Some(ref access_code) = options.access_code {
            // Pass both accessCode and linkCode (numeric privateServerLinkCode) — same as original RAM C#.
            // link_code should already be filtered to numeric-only by game.rs.
            let link_code = options.link_code.as_deref().unwrap_or("");
            format!(
                "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestPrivateGame&browserTrackerId={}&placeId={}&accessCode={}&linkCode={}",
                browser_tracker_id, options.place_id, access_code, link_code
            )
        } else if let Some(ref job_id) = options.job_id {
            format!(
                "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGameJob&browserTrackerId={}&placeId={}&gameId={}&isPlayTogetherGame=false",
                browser_tracker_id, options.place_id, job_id
            )
        } else if let Some(follow_id) = options.follow_user_id {
            format!(
                "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestFollowUser&browserTrackerId={}&userId={}&placeId={}",
                browser_tracker_id, follow_id, options.place_id
            )
        } else {
            format!(
                "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&browserTrackerId={}&placeId={}&isPlayTogetherGame=false",
                browser_tracker_id, options.place_id
            )
        };

        log::info!("PlaceLauncher URL: {}", launcher_url);

        // URL-encode the launcher URL
        let encoded_url = launcher_url
            .replace(':', "%3A")
            .replace('/', "%2F")
            .replace('?', "%3F")
            .replace('&', "%26")
            .replace('=', "%3D");

        // Build the roblox-player: protocol URI (legacy format, still works)
        let protocol_uri = format!(
            "roblox-player:1+launchmode:play+gameinfo:{}+launchtime:{}+placelauncherurl:{}+browsertrackerid:{}+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp",
            auth_ticket, timestamp, encoded_url, browser_tracker_id
        );

        // When a strap launcher (Fishstrap/Bloxstrap/Froststrap) is installed, it registers itself
        // as the roblox-player:// protocol handler and manages Roblox updates. Launching
        // RobloxPlayerBeta.exe directly bypasses that, so we skip to Method 2 instead.
        let strap_launcher_path: Option<PathBuf> = {
            let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
            ["Fishstrap", "Bloxstrap", "Froststrap"].iter().find_map(|name| {
                let exe = PathBuf::from(&local_app_data)
                    .join(name)
                    .join(format!("{}.exe", name));
                if exe.exists() {
                    log::info!("Strap launcher detected: {:?}", exe);
                    Some(exe)
                } else {
                    None
                }
            })
        };

        // Method 1: Direct exe launch with --app flag (like original RAM C#).
        // Only used when no strap launcher is managing Roblox — otherwise updates are skipped.
        if strap_launcher_path.is_none() {
            match Self::get_roblox_path() {
                Ok(roblox_path) => {
                    log::info!("Found Roblox at: {:?}", roblox_path);
                    let child = Command::new(&roblox_path)
                        .arg("--app")
                        .arg("-t")
                        .arg(auth_ticket)
                        .arg("-j")
                        .arg(&launcher_url)
                        .spawn();

                    match child {
                        Ok(child) => {
                            log::info!("Launched Roblox (PID: {}) for place {}", child.id(), options.place_id);
                            return Ok(child.id());
                        }
                        Err(e) => log::warn!("Direct exe spawn failed: {}, falling back to protocol handler", e),
                    }
                }
                Err(e) => log::warn!("Could not find Roblox path: {}, falling back to protocol handler", e),
            }
        }

        // Method 2: Open via OS protocol handler (roblox-player://).
        // When a strap launcher is installed it intercepts this URI, runs its update check,
        // and launches the correct Roblox version. For stock Roblox this is the fallback path.
        #[cfg(target_os = "windows")]
        {
            if let Some(ref strap) = strap_launcher_path {
                log::info!("Launching via strap launcher: {:?}", strap);
            }

            let child = Command::new("cmd")
                .args(["/C", "start", "", &protocol_uri])
                .spawn()
                .map_err(|e| {
                    AppError::Other(format!("Failed to launch Roblox via protocol handler: {}", e))
                })?;

            log::info!("Launched Roblox via protocol handler for place {}", options.place_id);
            return Ok(child.id());
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err(AppError::Other(
                "Roblox launching is only supported on Windows".into(),
            ))
        }
    }

    /// Resolve a privateServerLinkCode to an accessCode (UUID).
    /// Returns (access_code, Option<numeric_link_code>) — the numeric linkCode is needed by PlaceLauncher.
    /// Tries multiple methods:
    /// 1. Sharelinks API → privateServerId → private-servers list → accessCode
    /// 2. Game page HTML parsing on www.roblox.com
    /// 3. Game page HTML parsing on web.roblox.com (redirect fallback)
    pub async fn resolve_access_code(
        &self,
        cookie: &str,
        place_id: u64,
        link_code: &str,
    ) -> AppResult<(String, Option<String>)> {
        // Check cache first — another account may have already resolved this
        let cache_key = (place_id, link_code.to_string());
        if let Ok(cache) = ACCESS_CODE_CACHE.lock() {
            if let Some(cached) = cache.get(&cache_key) {
                log::info!("Using cached access code for place_id={}", place_id);
                return Ok(cached.clone());
            }
        }

        let cookie_header = format!(
            ".ROBLOSECURITY={}",
            cookie.trim_start_matches(".ROBLOSECURITY=")
        );

        // Detect code type:
        // - privateServerLinkCode: purely numeric, typically 10-20 digits (e.g. "12345678901234")
        // - share code: alphanumeric hex, typically 16+ chars (e.g. "a1b2c3d4e5f6a7b8")
        // - UUID access code: already resolved (e.g. "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx")
        let is_private_server_link_code = link_code.chars().all(|c| c.is_ascii_digit()) && link_code.len() >= 8;
        let is_uuid = link_code.len() == 36 && link_code.chars().filter(|&c| c == '-').count() == 4;

        // If it's already a UUID, it IS the access code
        if is_uuid {
            log::info!("link_code is already a UUID access code: {}", link_code);
            let result = (link_code.to_string(), None);
            if let Ok(mut cache) = ACCESS_CODE_CACHE.lock() {
                cache.insert(cache_key, result.clone());
            }
            return Ok(result);
        }

        log::info!(
            "Resolving access code: place_id={}, link_code={}, type={}",
            place_id, link_code,
            if is_private_server_link_code { "privateServerLinkCode" } else { "shareCode" }
        );

        if is_private_server_link_code {
            // Old-format privateServerLinkCode: use HTML parsing (original RAM method)
            // then fall back to sharelinks API
            self.resolve_private_server_link_code(&cookie_header, cookie, place_id, link_code, &cache_key).await
        } else {
            // New-format share code: use sharelinks API with retries
            // HTML parsing won't work for share codes
            self.resolve_share_code(&cookie_header, cookie, place_id, link_code, &cache_key).await
        }
    }

    /// Resolve access code from an old-format privateServerLinkCode (numeric).
    /// Uses HTML parsing first (like original RAM), then sharelinks API as fallback.
    async fn resolve_private_server_link_code(
        &self,
        cookie_header: &str,
        cookie: &str,
        place_id: u64,
        link_code: &str,
        cache_key: &(u64, String),
    ) -> AppResult<(String, Option<String>)> {
        // Try up to 3 attempts
        for attempt in 0..3 {
            if attempt > 0 {
                let delay = 500 * (1 << attempt); // 1000ms, 2000ms
                log::info!("Retry attempt {} after {}ms delay", attempt + 1, delay);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            let csrf = match self.get_csrf_token(cookie).await {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("Failed to get CSRF token (attempt {}): {}", attempt + 1, e);
                    continue;
                }
            };

            // For numeric link codes, the numeric_link_code is the link_code itself
            let nlc = Some(link_code.to_string());

            // Method 1: HTML parsing on www.roblox.com (original RAM method)
            match self.parse_access_code_from_page(
                &format!("https://www.roblox.com/games/{}?privateServerLinkCode={}", place_id, link_code),
                cookie_header, &csrf,
            ).await {
                Ok(code) => {
                    log::info!("Resolved via HTML (www): {}", code);
                    let result = (code, nlc);
                    if let Ok(mut cache) = ACCESS_CODE_CACHE.lock() {
                        cache.insert(cache_key.clone(), result.clone());
                    }
                    return Ok(result);
                }
                Err(e) => log::warn!("HTML parse (www) attempt {}: {}", attempt + 1, e),
            }

            // Method 2: HTML parsing on web.roblox.com (redirect fallback)
            match self.parse_access_code_from_page(
                &format!("https://web.roblox.com/games/{}?privateServerLinkCode={}", place_id, link_code),
                cookie_header, &csrf,
            ).await {
                Ok(code) => {
                    log::info!("Resolved via HTML (web): {}", code);
                    let result = (code, nlc);
                    if let Ok(mut cache) = ACCESS_CODE_CACHE.lock() {
                        cache.insert(cache_key.clone(), result.clone());
                    }
                    return Ok(result);
                }
                Err(e) => log::warn!("HTML parse (web) attempt {}: {}", attempt + 1, e),
            }

            // Method 3: Try sharelinks API as fallback (may work if Roblox maps old codes)
            match self.resolve_via_sharelinks_api(cookie_header, &csrf, place_id, link_code).await {
                Ok(code) => {
                    log::info!("Resolved via sharelinks API (fallback): {}", code);
                    let result = (code, nlc);
                    if let Ok(mut cache) = ACCESS_CODE_CACHE.lock() {
                        cache.insert(cache_key.clone(), result.clone());
                    }
                    return Ok(result);
                }
                Err(e) => log::warn!("Sharelinks API (fallback) attempt {}: {}", attempt + 1, e),
            }

            // Check cache — another concurrent call may have populated it
            if let Ok(cache) = ACCESS_CODE_CACHE.lock() {
                if let Some(cached) = cache.get(cache_key) {
                    log::info!("Found cached access code after attempt {}", attempt + 1);
                    return Ok(cached.clone());
                }
            }
        }

        Err(AppError::RobloxApi(
            "Could not resolve access code from privateServerLinkCode. The link may be invalid or expired.".into()
        ))
    }

    /// Resolve access code from a new-format share code (alphanumeric).
    /// Uses sharelinks API exclusively with exponential backoff retries.
    /// Returns (access_code, Option<numeric_link_code>).
    async fn resolve_share_code(
        &self,
        cookie_header: &str,
        cookie: &str,
        place_id: u64,
        link_code: &str,
        cache_key: &(u64, String),
    ) -> AppResult<(String, Option<String>)> {
        let mut last_error = String::new();

        // Try up to 4 attempts with exponential backoff
        for attempt in 0..4 {
            if attempt > 0 {
                let delay = 500 * (1 << attempt); // 1000ms, 2000ms, 4000ms
                log::info!("Share code retry attempt {} after {}ms delay", attempt + 1, delay);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            // Fresh CSRF token each attempt
            let csrf = match self.get_csrf_token(cookie).await {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("Failed to get CSRF token (attempt {}): {}", attempt + 1, e);
                    last_error = format!("CSRF token failed: {}", e);
                    continue;
                }
            };

            match self.resolve_via_sharelinks_api_with_status(cookie_header, &csrf, place_id, link_code).await {
                Ok((access_code, numeric_lc)) => {
                    log::info!("Resolved share code via sharelinks API (attempt {}): ac={}, nlc={:?}", attempt + 1, access_code, numeric_lc);
                    let result = (access_code, numeric_lc);
                    if let Ok(mut cache) = ACCESS_CODE_CACHE.lock() {
                        cache.insert(cache_key.clone(), result.clone());
                    }
                    return Ok(result);
                }
                Err((e, retryable)) => {
                    last_error = e.clone();
                    if retryable {
                        log::warn!("Sharelinks API transient failure (attempt {}): {}", attempt + 1, e);
                    } else {
                        log::error!("Sharelinks API permanent failure: {}", e);
                        break; // Don't retry on permanent errors (400, 404)
                    }
                }
            }

            // Check cache — another concurrent call may have populated it
            if let Ok(cache) = ACCESS_CODE_CACHE.lock() {
                if let Some(cached) = cache.get(cache_key) {
                    log::info!("Found cached access code after attempt {}", attempt + 1);
                    return Ok(cached.clone());
                }
            }
        }

        Err(AppError::RobloxApi(format!(
            "Could not resolve share code. Last error: {}. The link may be invalid or expired.", last_error
        )))
    }

    /// Resolve accessCode via the modern sharelinks API.
    /// Returns just the access_code string (used as fallback in resolve_private_server_link_code).
    async fn resolve_via_sharelinks_api(
        &self,
        cookie_header: &str,
        csrf: &str,
        place_id: u64,
        link_code: &str,
    ) -> AppResult<String> {
        match self.resolve_via_sharelinks_api_with_status(cookie_header, csrf, place_id, link_code).await {
            Ok((code, _)) => Ok(code),
            Err((msg, _)) => Err(AppError::RobloxApi(msg)),
        }
    }

    /// Sharelinks API resolution with internal CSRF retry.
    /// Returns Ok((access_code, Option<numeric_link_code>)) or Err((message, is_retryable)).
    async fn resolve_via_sharelinks_api_with_status(
        &self,
        cookie_header: &str,
        csrf: &str,
        place_id: u64,
        link_code: &str,
    ) -> Result<(String, Option<String>), (String, bool)> {
        let mut current_csrf = csrf.to_string();

        // Retry up to 3 times for CSRF token issues
        for csrf_attempt in 0..3 {
            let resp = self
                .client()
                .post("https://apis.roblox.com/sharelinks/v1/resolve-link")
                .header("Cookie", cookie_header)
                .header("x-csrf-token", &current_csrf)
                .header("Content-Type", "application/json")
                .header("Referer", "https://www.roblox.com")
                .header("Origin", "https://www.roblox.com")
                .json(&serde_json::json!({
                    "linkId": link_code,
                    "linkType": "Server"
                }))
                .send()
                .await
                .map_err(|e| (format!("HTTP error: {}", e), true))?;

            let status = resp.status();

            // Handle 403 — grab fresh CSRF from response header and retry
            if status.as_u16() == 403 {
                if let Some(new_csrf) = resp.headers().get("x-csrf-token").and_then(|v| v.to_str().ok()) {
                    log::info!("Sharelinks API got 403 with fresh CSRF on attempt {}, retrying", csrf_attempt + 1);
                    current_csrf = new_csrf.to_string();
                    continue;
                }
                // 403 without CSRF header — permanent auth failure
                return Err(("Authentication rejected (403, no CSRF in response)".into(), false));
            }

            let body = resp.text().await.unwrap_or_default();
            log::info!("Sharelinks API: status={}, body={}", status, &body[..body.len().min(500)]);

            // Classify HTTP status for retry logic
            if status.as_u16() == 429 {
                return Err(("Rate limited (429)".into(), true));
            }
            if status.is_server_error() {
                return Err((format!("Server error: {}", status), true));
            }
            if status.as_u16() == 400 || status.as_u16() == 404 {
                return Err((format!("Invalid link code ({}): {}", status, &body[..body.len().min(200)]), false));
            }
            if !status.is_success() {
                return Err((format!("Unexpected status {}: {}", status, &body[..body.len().min(200)]), true));
            }

            let json: serde_json::Value = serde_json::from_str(&body)
                .map_err(|_| (format!("Invalid JSON response: {}", &body[..body.len().min(200)]), false))?;

            // Try multiple response shapes — Roblox may change the structure
            let invite_data = json.get("privateServerInviteData")
                .or_else(|| json.get("linkData"))
                .or_else(|| json.get("data"));

            let invite_data = match invite_data {
                Some(d) => d,
                None => {
                    // If the top-level JSON itself has accessCode or privateServerId, use it directly
                    if let Some(ac) = json.get("accessCode").and_then(|v| v.as_str()) {
                        return Ok((ac.to_string(), None));
                    }
                    if let Some(lc) = json.get("linkCode").and_then(|v| v.as_str()) {
                        if lc.len() == 36 && lc.chars().filter(|&c| c == '-').count() == 4 {
                            return Ok((lc.to_string(), None));
                        }
                    }
                    return Err((format!("No invite data in response: {}", &body[..body.len().min(300)]), false));
                }
            };

            // Check if accessCode is directly available
            if let Some(ac) = invite_data.get("accessCode").and_then(|v| v.as_str()) {
                let nlc = invite_data.get("linkCode").and_then(|v| {
                    v.as_str().map(|s| s.to_string())
                        .or_else(|| v.as_u64().map(|n| n.to_string()))
                }).filter(|s| s.chars().all(|c| c.is_ascii_digit()));
                return Ok((ac.to_string(), nlc));
            }

            // Check linkCode field — Roblox returns the privateServerLinkCode here.
            if let Some(lc) = invite_data.get("linkCode").and_then(|v| {
                v.as_str().map(|s| s.to_string())
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            }) {
                // If it's a UUID, it's the accessCode directly
                if lc.len() == 36 && lc.chars().filter(|&c| c == '-').count() == 4 {
                    log::info!("Sharelinks returned linkCode as UUID accessCode: {}", lc);
                    return Ok((lc, None));
                }
                // It's a numeric privateServerLinkCode — resolve via HTML parsing
                if !lc.is_empty() {
                    log::info!("Sharelinks returned numeric linkCode={}, resolving via HTML parsing", lc);
                    let numeric_lc = Some(lc.clone());
                    let resolved_place_id = invite_data.get("placeId")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(place_id);

                    // Try HTML parsing with the numeric linkCode
                    if let Ok(access_code) = self.parse_access_code_from_page(
                        &format!("https://www.roblox.com/games/{}?privateServerLinkCode={}", resolved_place_id, lc),
                        cookie_header, &current_csrf,
                    ).await {
                        log::info!("Resolved accessCode via HTML from sharelinks linkCode: {}", access_code);
                        return Ok((access_code, numeric_lc));
                    }
                    // Try web.roblox.com fallback
                    if let Ok(access_code) = self.parse_access_code_from_page(
                        &format!("https://web.roblox.com/games/{}?privateServerLinkCode={}", resolved_place_id, lc),
                        cookie_header, &current_csrf,
                    ).await {
                        log::info!("Resolved accessCode via HTML (web) from sharelinks linkCode: {}", access_code);
                        return Ok((access_code, numeric_lc));
                    }
                    log::warn!("HTML parsing failed for sharelinks linkCode={}", lc);
                }
            }

            // Fallback: Get privateServerId and resolve accessCode from private-servers list
            let ps_id = invite_data.get("privateServerId")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ("No privateServerId, accessCode, or linkCode in sharelinks response".to_string(), false))?;

            return self.resolve_access_code_by_server_id(cookie_header, &current_csrf, place_id, ps_id)
                .await
                .map(|ac| (ac, None))
                .map_err(|e| (format!("Failed to resolve by server ID: {}", e), true));
        }

        Err(("CSRF token rejected after 3 retries".into(), false))
    }

    /// Fetch accessCode for a private server by its privateServerId.
    async fn resolve_access_code_by_server_id(
        &self,
        cookie_header: &str,
        csrf: &str,
        place_id: u64,
        private_server_id: u64,
    ) -> AppResult<String> {
        // Try private-servers list first
        let resp = self
            .client()
            .get(format!(
                "https://games.roblox.com/v1/games/{}/private-servers?limit=100",
                place_id
            ))
            .header("Cookie", cookie_header)
            .header("x-csrf-token", csrf)
            .header("Referer", "https://www.roblox.com")
            .send()
            .await?;

        let body = resp.text().await.unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(servers) = json.get("data").and_then(|v| v.as_array()) {
                for server in servers {
                    let vid = server.get("vipServerId").and_then(|v| v.as_u64()).unwrap_or(0);
                    if vid == private_server_id {
                        if let Some(ac) = server.get("accessCode").and_then(|v| v.as_str()) {
                            return Ok(ac.to_string());
                        }
                    }
                }
            }
        }

        // Fallback: direct VIP server endpoint
        let resp = self
            .client()
            .get(format!(
                "https://games.roblox.com/v1/vip-servers/{}",
                private_server_id
            ))
            .header("Cookie", cookie_header)
            .header("x-csrf-token", csrf)
            .header("Referer", "https://www.roblox.com")
            .send()
            .await?;

        let body = resp.text().await.unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(ac) = json.get("accessCode").and_then(|v| v.as_str()) {
                return Ok(ac.to_string());
            }
        }

        Err(AppError::RobloxApi(format!(
            "Could not find accessCode for privateServerId={}", private_server_id
        )))
    }

    /// Parse access code from a Roblox game page HTML (legacy method).
    async fn parse_access_code_from_page(
        &self,
        url: &str,
        cookie_header: &str,
        csrf: &str,
    ) -> AppResult<String> {
        let resp = self
            .client()
            .get(url)
            .header("Cookie", cookie_header)
            .header("x-csrf-token", csrf)
            .header("Referer", "https://www.roblox.com/games/4924922222/Brookhaven-RP")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi(format!(
                "Game page returned status: {}", resp.status()
            )));
        }

        let body = resp.text().await?;

        // Pattern 1: Roblox.GameLauncher.joinPrivateGame(placeId, 'UUID')
        let re = r"Roblox\.GameLauncher\.joinPrivateGame\(\d+,\s*'([0-9a-fA-F\-]{36})'";
        if let Some(caps) = regex_lite::Regex::new(re).ok().and_then(|r| r.captures(&body)) {
            if let Some(m) = caps.get(1) {
                return Ok(m.as_str().to_string());
            }
        }

        // Pattern 2: "accessCode":"UUID" in embedded JSON
        let re2 = r#""accessCode"\s*:\s*"([0-9a-fA-F\-]{36})""#;
        if let Some(caps) = regex_lite::Regex::new(re2).ok().and_then(|r| r.captures(&body)) {
            if let Some(m) = caps.get(1) {
                return Ok(m.as_str().to_string());
            }
        }

        Err(AppError::RobloxApi("Access code not found in page HTML".into()))
    }

    /// Launch Roblox to join a share link directly via protocol handler.
    pub fn launch_share_link(share_code: &str) -> AppResult<u32> {
        let uri = format!(
            "roblox://navigation/share_links?type=Server&code={}",
            share_code
        );

        #[cfg(target_os = "windows")]
        {
            let child = Command::new("cmd")
                .args(["/C", "start", "", &uri])
                .spawn()
                .map_err(|e| {
                    AppError::Other(format!("Failed to open share link: {}", e))
                })?;

            return Ok(child.id());
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err(AppError::Other(
                "Roblox launching is only supported on Windows".into(),
            ))
        }
    }
}
