use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, AppResult};
use crate::models::Account;
use crate::state::AppState;

/// Original RAM C# AccountData.json format
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LegacyAccount {
    /// Whether the account is valid
    #[serde(default)]
    valid: bool,
    /// The .ROBLOSECURITY cookie (may be encrypted with DPAPI)
    #[serde(default)]
    security_token: Option<String>,
    /// Roblox username
    #[serde(default)]
    username: Option<String>,
    /// Last use timestamp
    #[serde(default)]
    last_use: Option<String>,
    /// Account alias
    #[serde(default)]
    alias: Option<String>,
    /// Account description
    #[serde(default)]
    description: Option<String>,
    /// Stored password (we won't import this)
    #[serde(default)]
    password: Option<String>,
    /// Group name
    #[serde(default)]
    group: Option<String>,
    /// Roblox user ID
    #[serde(default, rename = "UserID")]
    user_id: Option<u64>,
    /// Custom fields
    #[serde(default)]
    fields: Option<std::collections::HashMap<String, String>>,
}

/// Result of importing a single legacy account
#[derive(Debug, Clone, Serialize)]
pub struct LegacyImportResult {
    pub index: usize,
    pub success: bool,
    pub username: Option<String>,
    pub error: Option<String>,
    /// Whether the cookie was validated online
    pub validated: bool,
}

/// Progress event for legacy import
#[derive(Debug, Clone, Serialize)]
pub struct LegacyImportProgress {
    pub current: usize,
    pub total: usize,
    pub result: LegacyImportResult,
}

/// Import accounts from original RAM AccountData.json format
/// 
/// The original app uses DPAPI encryption by default (Windows-only, machine-specific).
/// We can only import accounts whose cookies are in plain text or have been decrypted.
/// If a cookie starts with "_|WARNING:" it's a valid Roblox cookie.
/// If it's gibberish/binary, it's likely DPAPI-encrypted and we skip it.
#[tauri::command]
pub async fn import_legacy_accounts(
    app: AppHandle,
    json_content: String,
    validate_online: bool,
    state: State<'_, AppState>,
) -> AppResult<Vec<LegacyImportResult>> {
    // Parse the JSON array
    let legacy_accounts: Vec<LegacyAccount> = serde_json::from_str(&json_content)
        .map_err(|e| AppError::Other(format!(
            "Failed to parse AccountData.json: {}. Make sure this is the original RAM format.",
            e
        )))?;

    let mut results = Vec::new();
    let total = legacy_accounts.len();

    for (index, legacy) in legacy_accounts.iter().enumerate() {
        let username = legacy.username.clone().unwrap_or_else(|| format!("Account #{}", index));

        // Helper: emit progress and push result
        macro_rules! emit_and_push {
            ($result:expr) => {{
                let r: LegacyImportResult = $result;
                let _ = app.emit("legacy-import-progress", LegacyImportProgress {
                    current: results.len() + 1,
                    total,
                    result: r.clone(),
                });
                results.push(r);
            }};
        }

        // Check if we have a security token
        let cookie = match &legacy.security_token {
            Some(token) if !token.is_empty() => {
                let clean = token
                    .trim()
                    .trim_start_matches("_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you.--")
                    .trim_start_matches(".ROBLOSECURITY=");

                if clean.len() < 50 {
                    emit_and_push!(LegacyImportResult {
                        index,
                        success: false,
                        username: Some(username),
                        error: Some("Cookie too short - may be DPAPI encrypted. Export from original RAM with 'No Encryption' first.".into()),
                        validated: false,
                    });
                    continue;
                }

                if !clean.is_ascii() {
                    emit_and_push!(LegacyImportResult {
                        index,
                        success: false,
                        username: Some(username),
                        error: Some("Cookie appears to be DPAPI encrypted. Export from original RAM with 'No Encryption' first.".into()),
                        validated: false,
                    });
                    continue;
                }

                clean.to_string()
            }
            _ => {
                emit_and_push!(LegacyImportResult {
                    index,
                    success: false,
                    username: Some(username),
                    error: Some("No security token found".into()),
                    validated: false,
                });
                continue;
            }
        };

        if validate_online {
            match state.roblox.get_authenticated_user(&cookie).await {
                Ok(user) => {
                    let mut account = Account::new(user.name.clone());
                    account.user_id = Some(user.id);
                    account.display_name = Some(user.display_name);
                    account.group = legacy.group.clone();
                    account.alias = legacy.alias.clone();
                    account.description = legacy.description.clone();

                    if let Some(fields) = &legacy.fields {
                        account.fields = fields.clone();
                    }

                    if let Some(last_use) = &legacy.last_use {
                        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(last_use) {
                            account.last_used = Some(dt.with_timezone(&chrono::Utc));
                        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(last_use, "%Y-%m-%dT%H:%M:%S") {
                            account.last_used = Some(dt.and_utc());
                        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(last_use, "%m/%d/%Y %I:%M:%S %p") {
                            account.last_used = Some(dt.and_utc());
                        }
                    }

                    let (robux, premium, avatar_url) = tokio::join!(
                        state.roblox.get_robux(user.id, &cookie),
                        state.roblox.is_premium(user.id, &cookie),
                        state.roblox.get_avatar_url(user.id),
                    );

                    account.robux = robux.ok();
                    account.is_premium = premium.ok();
                    account.avatar_url = avatar_url.ok().flatten();

                    let display_name = account.username.clone();

                    let mut store = state.store.lock().map_err(|e| {
                        AppError::Other(format!("Lock poisoned: {}", e))
                    })?;

                    match store.add_account(account, &cookie) {
                        Ok(()) => {
                            emit_and_push!(LegacyImportResult {
                                index,
                                success: true,
                                username: Some(display_name),
                                error: None,
                                validated: true,
                            });
                        }
                        Err(e) => {
                            emit_and_push!(LegacyImportResult {
                                index,
                                success: false,
                                username: Some(display_name),
                                error: Some(e.to_string()),
                                validated: true,
                            });
                        }
                    }
                }
                Err(e) => {
                    emit_and_push!(LegacyImportResult {
                        index,
                        success: false,
                        username: Some(username),
                        error: Some(format!("Cookie validation failed: {}", e)),
                        validated: false,
                    });
                }
            }
        } else {
            let mut account = Account::new(username.clone());
            account.user_id = legacy.user_id;
            account.group = legacy.group.clone();
            account.alias = legacy.alias.clone();
            account.description = legacy.description.clone();

            if let Some(fields) = &legacy.fields {
                account.fields = fields.clone();
            }

            if let Some(last_use) = &legacy.last_use {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(last_use) {
                    account.last_used = Some(dt.with_timezone(&chrono::Utc));
                } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(last_use, "%Y-%m-%dT%H:%M:%S") {
                    account.last_used = Some(dt.and_utc());
                } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(last_use, "%m/%d/%Y %I:%M:%S %p") {
                    account.last_used = Some(dt.and_utc());
                }
            }

            let mut store = state.store.lock().map_err(|e| {
                AppError::Other(format!("Lock poisoned: {}", e))
            })?;

            match store.add_account(account, &cookie) {
                Ok(()) => {
                    emit_and_push!(LegacyImportResult {
                        index,
                        success: true,
                        username: Some(username),
                        error: None,
                        validated: false,
                    });
                }
                Err(e) => {
                    emit_and_push!(LegacyImportResult {
                        index,
                        success: false,
                        username: Some(username),
                        error: Some(e.to_string()),
                        validated: false,
                    });
                }
            }
        }
    }

    Ok(results)
}
