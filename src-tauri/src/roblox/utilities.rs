use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::RobloxClient;

/// Privacy settings for an account
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySettings {
    pub phone_discovery: Option<String>,
    pub private_message_privacy: Option<String>,
    pub app_chat_privacy: Option<String>,
    pub game_chat_privacy: Option<String>,
    pub inventory_privacy: Option<String>,
    pub trade_privacy: Option<String>,
    pub trade_value: Option<String>,
}

/// Gender type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenderResponse {
    pub gender: u8, // 1=unknown, 2=male, 3=female
}

/// Birthdate
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BirthdateResponse {
    pub birth_month: u32,
    pub birth_day: u32,
    pub birth_year: u32,
}

impl RobloxClient {
    /// Change password for an account
    pub async fn change_password(
        &self,
        cookie: &str,
        old_password: &str,
        new_password: &str,
    ) -> AppResult<()> {
        let csrf = self.get_csrf_token(cookie).await?;
        let cookie_header = Self::format_cookie(cookie);

        let body = serde_json::json!({
            "currentPassword": old_password,
            "newPassword": new_password,
        });

        let resp = self
            .client()
            .post("https://auth.roblox.com/v2/user/passwords/change")
            .header("Cookie", &cookie_header)
            .header("x-csrf-token", &csrf)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::RobloxApi(format!(
                "Failed to change password ({}): {}",
                status, text
            )));
        }

        Ok(())
    }

    /// Change email for an account
    pub async fn change_email(
        &self,
        cookie: &str,
        password: &str,
        new_email: &str,
    ) -> AppResult<()> {
        let csrf = self.get_csrf_token(cookie).await?;
        let cookie_header = Self::format_cookie(cookie);

        let body = serde_json::json!({
            "password": password,
            "emailAddress": new_email,
        });

        let resp = self
            .client()
            .post("https://accountsettings.roblox.com/v1/email")
            .header("Cookie", &cookie_header)
            .header("x-csrf-token", &csrf)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::RobloxApi(format!(
                "Failed to change email ({}): {}",
                status, text
            )));
        }

        Ok(())
    }

    /// Get privacy settings
    pub async fn get_privacy_settings(&self, cookie: &str) -> AppResult<PrivacySettings> {
        let cookie_header = Self::format_cookie(cookie);

        let resp = self
            .client()
            .get("https://accountsettings.roblox.com/v1/app-chat-privacy")
            .header("Cookie", &cookie_header)
            .send()
            .await?;

        // Build privacy settings from multiple endpoints
        let app_chat = if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await?;
            data.get("appChatPrivacy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        let resp = self
            .client()
            .get("https://accountsettings.roblox.com/v1/game-chat-privacy")
            .header("Cookie", &cookie_header)
            .send()
            .await?;

        let game_chat = if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await?;
            data.get("gameChatPrivacy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        let resp = self
            .client()
            .get("https://accountsettings.roblox.com/v1/inventory-privacy")
            .header("Cookie", &cookie_header)
            .send()
            .await?;

        let inventory = if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await?;
            data.get("inventoryPrivacy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        let resp = self
            .client()
            .get("https://accountsettings.roblox.com/v1/trade-privacy")
            .header("Cookie", &cookie_header)
            .send()
            .await?;

        let (trade, trade_value) = if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await?;
            (
                data.get("tradePrivacy")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                data.get("tradeValue")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            )
        } else {
            (None, None)
        };

        let resp = self
            .client()
            .get("https://accountsettings.roblox.com/v1/private-message-privacy")
            .header("Cookie", &cookie_header)
            .send()
            .await?;

        let private_message = if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await?;
            data.get("privateMessagePrivacy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        Ok(PrivacySettings {
            phone_discovery: None,
            private_message_privacy: private_message,
            app_chat_privacy: app_chat,
            game_chat_privacy: game_chat,
            inventory_privacy: inventory,
            trade_privacy: trade,
            trade_value,
        })
    }

    /// Set description/about for an account
    pub async fn set_description(
        &self,
        cookie: &str,
        description: &str,
    ) -> AppResult<()> {
        let csrf = self.get_csrf_token(cookie).await?;
        let cookie_header = Self::format_cookie(cookie);

        let body = serde_json::json!({
            "description": description,
        });

        let resp = self
            .client()
            .post("https://accountinformation.roblox.com/v1/description")
            .header("Cookie", &cookie_header)
            .header("x-csrf-token", &csrf)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::RobloxApi(format!(
                "Failed to set description ({}): {}",
                status, text
            )));
        }

        Ok(())
    }

    /// Set display name
    pub async fn set_display_name(
        &self,
        cookie: &str,
        user_id: u64,
        new_display_name: &str,
    ) -> AppResult<()> {
        let csrf = self.get_csrf_token(cookie).await?;
        let cookie_header = Self::format_cookie(cookie);

        let body = serde_json::json!({
            "newDisplayName": new_display_name,
        });

        let resp = self
            .client()
            .patch(format!(
                "https://users.roblox.com/v1/users/{}/display-names",
                user_id
            ))
            .header("Cookie", &cookie_header)
            .header("x-csrf-token", &csrf)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::RobloxApi(format!(
                "Failed to set display name ({}): {}",
                status, text
            )));
        }

        Ok(())
    }

    /// Get gender
    pub async fn get_gender(&self, cookie: &str) -> AppResult<u8> {
        let cookie_header = Self::format_cookie(cookie);

        let resp = self
            .client()
            .get("https://accountinformation.roblox.com/v1/gender")
            .header("Cookie", &cookie_header)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi("Failed to get gender".into()));
        }

        let data: GenderResponse = resp.json().await?;
        Ok(data.gender)
    }

    /// Get birthdate
    pub async fn get_birthdate(&self, cookie: &str) -> AppResult<BirthdateResponse> {
        let cookie_header = Self::format_cookie(cookie);

        let resp = self
            .client()
            .get("https://accountinformation.roblox.com/v1/birthdate")
            .header("Cookie", &cookie_header)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::RobloxApi("Failed to get birthdate".into()));
        }

        let data: BirthdateResponse = resp.json().await?;
        Ok(data)
    }

    /// Format cookie for header
    fn format_cookie(cookie: &str) -> String {
        if cookie.starts_with(".ROBLOSECURITY=") {
            cookie.to_string()
        } else {
            format!(".ROBLOSECURITY={}", cookie)
        }
    }
}
