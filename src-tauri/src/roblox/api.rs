use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};
use serde::Deserialize;

use crate::error::{AppError, AppResult};

const ROBLOX_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

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
        Self {
            client: reqwest::Client::builder()
                .user_agent(ROBLOX_USER_AGENT)
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

    /// Get CSRF token for POST requests.
    /// Uses a lightweight endpoint that returns 403 with x-csrf-token header
    /// without side effects (no logout, no ticket generation).
    pub async fn get_csrf_token(&self, cookie: &str) -> AppResult<String> {
        // Use /v1/account/pin/ which is a safe read-only endpoint that returns CSRF on 403
        let resp = self
            .client
            .post("https://auth.roblox.com/v1/account/pin/")
            .headers(Self::auth_headers(cookie))
            .header("Referer", "https://www.roblox.com")
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
            .header("Referer", "https://www.roblox.com")
            .send()
            .await?;

        resp.headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::RobloxApi("Failed to get CSRF token".into()))
    }
}
