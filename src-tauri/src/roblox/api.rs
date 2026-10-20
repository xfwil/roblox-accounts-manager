use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT, ACCEPT, ACCEPT_LANGUAGE};
use serde::Deserialize;

use crate::error::{AppError, AppResult};

/// Mimics a real browser user-agent to avoid CAPTCHA/bot detection.
/// Using a modern Chrome on Windows UA — update periodically.
const ROBLOX_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Authenticated user info from Roblox
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedUser {
    pub id: u64,
    pub name: String,
    pub display_name: String,
}

/// User details from users API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDetails {
    pub description: Option<String>,
    pub created: Option<String>,
    pub is_banned: Option<bool>,
    pub has_verified_badge: Option<bool>,
}

/// Robux balance
#[derive(Debug, Deserialize)]
pub struct CurrencyResponse {
    pub robux: i64,
}

/// Premium status
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PremiumResponse {
    // The endpoint returns 200 if premium, 403 if not
}

/// Roblox API client
pub struct RobloxClient {
    client: reqwest::Client,
}

impl RobloxClient {
    pub fn new() -> Self {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"));
        default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        default_headers.insert(
            reqwest::header::HeaderName::from_static("sec-ch-ua"),
            HeaderValue::from_static("\"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\""),
        );
        default_headers.insert(
            reqwest::header::HeaderName::from_static("sec-ch-ua-mobile"),
            HeaderValue::from_static("?0"),
        );
        default_headers.insert(
            reqwest::header::HeaderName::from_static("sec-ch-ua-platform"),
            HeaderValue::from_static("\"Windows\""),
        );

        Self {
            client: reqwest::Client::builder()
                .user_agent(ROBLOX_USER_AGENT)
                .default_headers(default_headers)
                .cookie_store(false) // Don't share cookies between requests
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Access the underlying reqwest client
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Build headers with the .ROBLOSECURITY cookie
    pub(crate) fn auth_headers(cookie: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let cookie_value = if cookie.starts_with(".ROBLOSECURITY=") {
            cookie.to_string()
        } else {
            format!(".ROBLOSECURITY={}", cookie)
        };
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&cookie_value).unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(ROBLOX_USER_AGENT),
        );
        headers
    }

    /// Validate a cookie and get the authenticated user
    pub async fn get_authenticated_user(&self, cookie: &str) -> AppResult<AuthenticatedUser> {
        let resp = self
            .client
            .get("https://users.roblox.com/v1/users/authenticated")
            .headers(Self::auth_headers(cookie))
            .send()
            .await?;

        if resp.status() == 401 {
            return Err(AppError::InvalidCookie("Cookie is invalid or expired".into()));
        }

        // A moderated/banned account keeps a valid cookie but Roblox answers 403
        // with `{"errors":[{"message":"User is moderated"}]}` on authenticated
        // endpoints. Distinguish it from a plain forbidden so we can flag it.
        if resp.status() == 403 {
            let body = resp.text().await.unwrap_or_default();
            if body.contains("moderated") {
                return Err(AppError::Moderated("User is moderated".into()));
            }
            return Err(AppError::RobloxApi(format!("Forbidden (403): {}", body)));
        }

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi(format!(
                "Failed to get authenticated user: {}",
                resp.status()
            )));
        }

        let user: AuthenticatedUser = resp.json().await?;
        Ok(user)
    }

    /// Get detailed user info
    pub async fn get_user_details(&self, user_id: u64, cookie: &str) -> AppResult<UserDetails> {
        let resp = self
            .client
            .get(format!("https://users.roblox.com/v1/users/{}", user_id))
            .headers(Self::auth_headers(cookie))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi(format!(
                "Failed to get user details: {}",
                resp.status()
            )));
        }

        let details: UserDetails = resp.json().await?;
        Ok(details)
    }

    /// Get Robux balance
    pub async fn get_robux(&self, user_id: u64, cookie: &str) -> AppResult<i64> {
        let resp = self
            .client
            .get(format!(
                "https://economy.roblox.com/v1/users/{}/currency",
                user_id
            ))
            .headers(Self::auth_headers(cookie))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi(format!(
                "Failed to get Robux balance: {}",
                resp.status()
            )));
        }

        let currency: CurrencyResponse = resp.json().await?;
        Ok(currency.robux)
    }

    /// Check if user has Premium
    pub async fn is_premium(&self, user_id: u64, cookie: &str) -> AppResult<bool> {
        let resp = self
            .client
            .get(format!(
                "https://premiumfeatures.roblox.com/v1/users/{}/validate-membership",
                user_id
            ))
            .headers(Self::auth_headers(cookie))
            .send()
            .await?;

        // Returns true/false as body
        if !resp.status().is_success() {
            return Ok(false);
        }

        let text = resp.text().await?;
        Ok(text.trim() == "true")
    }

    /// Get avatar thumbnail URL
    pub async fn get_avatar_url(&self, user_id: u64) -> AppResult<Option<String>> {
        #[derive(Deserialize)]
        struct ThumbnailData {
            #[serde(rename = "imageUrl")]
            image_url: Option<String>,
        }
        #[derive(Deserialize)]
        struct ThumbnailResponse {
            data: Vec<ThumbnailData>,
        }

        let resp = self
            .client
            .get(format!(
                "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=150x150&format=Png",
                user_id
            ))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let result: ThumbnailResponse = resp.json().await?;
        Ok(result.data.first().and_then(|d| d.image_url.clone()))
    }

    /// Public (no cookie) permanent-ban check: `isBanned` on users/{id}.
    /// Every authenticated API just says "moderated"; this tells a permanent
    /// ban (true) from an unlockable one (false). Looks the id up by name if unknown.
    pub async fn is_banned(&self, user_id: Option<u64>, username: &str) -> AppResult<bool> {
        #[derive(Deserialize)]
        struct Found { id: u64 }
        #[derive(Deserialize)]
        struct Lookup { data: Vec<Found> }
        let id = match user_id {
            Some(id) => id,
            None => {
                let r: Lookup = self.client
                    .post("https://users.roblox.com/v1/usernames/users")
                    .json(&serde_json::json!({ "usernames": [username], "excludeBannedUsers": false }))
                    .send().await?.json().await?;
                r.data.first().map(|f| f.id)
                    .ok_or_else(|| AppError::RobloxApi(format!("User {username} not found")))?
            }
        };
        // Public endpoint is rate-limited; bulk refresh can hit 429, so back off and retry.
        let mut last = String::new();
        for attempt in 0..4u64 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(2 * attempt)).await;
            }
            let resp = self.client
                .get(format!("https://users.roblox.com/v1/users/{id}"))
                .header(ACCEPT, "application/json")
                .send().await?;
            if resp.status().is_success() {
                let d: UserDetails = resp.json().await?;
                return Ok(d.is_banned.unwrap_or(false));
            }
            last = resp.status().to_string();
        }
        Err(AppError::RobloxApi(format!("Ban check failed: {last}")))
    }

    /// Press "Unlock" on a moderated account (same call as roblox.com/not-approved).
    /// Returns (status, challenge type, body). A challenge comes back as 403 with
    /// `rblx-challenge-type`; a 2xx with no challenge means the account was unlocked.
    pub async fn press_unlock(&self, cookie: &str) -> AppResult<(u16, Option<String>, String)> {
        let url = "https://usermoderation.roblox.com/v1/not-approved/reactivate";
        let mut token = String::new();
        for _ in 0..2 {
            let resp = self
                .client
                .post(url)
                .headers(Self::auth_headers(cookie))
                .header("x-csrf-token", &token)
                .header("Origin", "https://www.roblox.com")
                .header("Referer", "https://www.roblox.com/not-approved")
                .header("Content-Type", "application/json")
                .body("{}")
                .send()
                .await?;
            let status = resp.status().as_u16();
            let h = |k: &str| resp.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
            let (challenge, csrf) = (h("rblx-challenge-type"), h("x-csrf-token"));
            if status == 403 && challenge.is_none() && token.is_empty() {
                if let Some(t) = csrf {
                    token = t; // first call only fetches the CSRF token
                    continue;
                }
            }
            let body = resp.text().await.unwrap_or_default();
            return Ok((status, challenge, body));
        }
        Err(AppError::RobloxApi("Unlock: CSRF token rejected".into()))
    }

    /// Get CSRF token for POST requests.
    /// Uses a lightweight endpoint that returns 403 with x-csrf-token header
    /// without side effects (no logout, no ticket generation).
    pub async fn get_csrf_token(&self, cookie: &str) -> AppResult<String> {
        // Use /v1/account/pin/ which is a safe read-only endpoint that returns CSRF on 403
        let resp = self
            .client
            .post("https://auth.roblox.com/v1/account/pin/")
            .headers(Self::auth_headers(cookie))
            .header("Origin", "https://www.roblox.com")
            .header("Referer", "https://www.roblox.com/")
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-site")
            .send()
            .await?;

        if let Some(token) = resp.headers().get("x-csrf-token").and_then(|v| v.to_str().ok()) {
            return Ok(token.to_string());
        }

        // Fallback: try with a different endpoint
        let resp = self
            .client
            .post("https://friends.roblox.com/v1/users/1/request-friendship")
            .headers(Self::auth_headers(cookie))
            .header("Origin", "https://www.roblox.com")
            .header("Referer", "https://www.roblox.com/")
            .header("Sec-Fetch-Dest", "empty")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Site", "same-site")
            .send()
            .await?;

        resp.headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::RobloxApi("Failed to get CSRF token".into()))
    }
}
