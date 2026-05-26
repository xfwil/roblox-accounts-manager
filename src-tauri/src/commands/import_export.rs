use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, AppResult};
use crate::models::Account;
use crate::state::AppState;

/// Import result for a single entry
#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub index: usize,
    pub success: bool,
    pub username: Option<String>,
    pub error: Option<String>,
}

/// Progress event emitted per-account during import
#[derive(Debug, Clone, Serialize)]
pub struct ImportProgress {
    pub current: usize,
    pub total: usize,
    pub result: ImportResult,
}

/// Export format for accounts
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportedAccount {
    pub username: String,
    pub cookie: String,
    pub group: Option<String>,
    pub alias: Option<String>,
}

/// Import accounts from cookies (one per line)
#[tauri::command]
pub async fn import_cookies(
    app: AppHandle,
    cookies: Vec<String>,
    group: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<ImportResult>> {
    let mut results = Vec::new();
    let total = cookies.iter().filter(|c| !c.trim().is_empty()).count();

    for (index, cookie) in cookies.iter().enumerate() {
        let cookie = cookie.trim();
        if cookie.is_empty() {
            continue;
        }

        // Strip the prefix if present
        let clean_cookie = cookie
            .trim_start_matches("_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you.--")
            .trim_start_matches(".ROBLOSECURITY=");
        let clean_cookie = if clean_cookie == cookie {
            cookie.to_string()
        } else if cookie.starts_with(".ROBLOSECURITY=") {
            cookie.trim_start_matches(".ROBLOSECURITY=").to_string()
        } else {
            cookie.to_string()
        };

        let result = match state.roblox.get_authenticated_user(&clean_cookie).await {
            Ok(user) => {
                let mut account = Account::new(user.name.clone());
                account.user_id = Some(user.id);
                account.display_name = Some(user.display_name);
                account.group = group.clone();

                // Fetch additional info (best effort)
                let (robux, premium, avatar_url) = tokio::join!(
                    state.roblox.get_robux(user.id, &clean_cookie),
                    state.roblox.is_premium(user.id, &clean_cookie),
                    state.roblox.get_avatar_url(user.id),
                );

                account.robux = robux.ok();
                account.is_premium = premium.ok();
                account.avatar_url = avatar_url.ok().flatten();

                let username = account.username.clone();

                let mut store = state.store.lock().map_err(|e| {
                    AppError::Other(format!("Lock poisoned: {}", e))
                })?;
                match store.add_account(account, &clean_cookie) {
                    Ok(()) => ImportResult {
                        index,
                        success: true,
                        username: Some(username),
                        error: None,
                    },
                    Err(e) => ImportResult {
                        index,
                        success: false,
                        username: Some(username),
                        error: Some(e.to_string()),
                    },
                }
            }
            Err(e) => ImportResult {
                index,
                success: false,
                username: None,
                error: Some(e.to_string()),
            },
        };

        // Emit progress event
        let _ = app.emit("import-progress", ImportProgress {
            current: results.len() + 1,
            total,
            result: result.clone(),
        });

        results.push(result);
    }

    Ok(results)
}

/// Import accounts from user:pass:cookie format
#[tauri::command]
pub async fn import_user_pass_cookie(
    app: AppHandle,
    entries: Vec<String>,
    group: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<Vec<ImportResult>> {
    let mut results = Vec::new();
    let total = entries.iter().filter(|e| !e.trim().is_empty()).count();

    for (index, entry) in entries.iter().enumerate() {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        // Parse user:pass:cookie or user:cookie format
        let parts: Vec<&str> = entry.splitn(3, ':').collect();
        let cookie = match parts.len() {
            3 => parts[2], // user:pass:cookie
            2 => parts[1], // user:cookie
            _ => {
                let result = ImportResult {
                    index,
                    success: false,
                    username: None,
                    error: Some("Invalid format. Expected user:pass:cookie or user:cookie".into()),
                };
                let _ = app.emit("import-progress", ImportProgress {
                    current: results.len() + 1,
                    total,
                    result: result.clone(),
                });
                results.push(result);
                continue;
            }
        };

        let result = match state.roblox.get_authenticated_user(cookie).await {
            Ok(user) => {
                let mut account = Account::new(user.name.clone());
                account.user_id = Some(user.id);
                account.display_name = Some(user.display_name);
                account.group = group.clone();

                let (robux, premium, avatar_url) = tokio::join!(
                    state.roblox.get_robux(user.id, cookie),
                    state.roblox.is_premium(user.id, cookie),
                    state.roblox.get_avatar_url(user.id),
                );

                account.robux = robux.ok();
                account.is_premium = premium.ok();
                account.avatar_url = avatar_url.ok().flatten();

                let username = account.username.clone();

                let mut store = state.store.lock().map_err(|e| {
                    AppError::Other(format!("Lock poisoned: {}", e))
                })?;
                match store.add_account(account, cookie) {
                    Ok(()) => ImportResult {
                        index,
                        success: true,
                        username: Some(username),
                        error: None,
                    },
                    Err(e) => ImportResult {
                        index,
                        success: false,
                        username: Some(username),
                        error: Some(e.to_string()),
                    },
                }
            }
            Err(e) => ImportResult {
                index,
                success: false,
                username: Some(parts[0].to_string()),
                error: Some(e.to_string()),
            },
        };

        let _ = app.emit("import-progress", ImportProgress {
            current: results.len() + 1,
            total,
            result: result.clone(),
        });

        results.push(result);
    }

    Ok(results)
}

/// Export all accounts with their cookies
#[tauri::command]
pub async fn export_accounts(
    format: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let store = state.store.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;

    let accounts = store.list_accounts();
    let mut output = String::new();

    match format.as_str() {
        "cookies" => {
            // One cookie per line
            for account in &accounts {
                if let Ok(cookie) = store.get_cookie(&account.id) {
                    output.push_str(&cookie);
                    output.push('\n');
                }
            }
        }
        "user:cookie" => {
            for account in &accounts {
                if let Ok(cookie) = store.get_cookie(&account.id) {
                    output.push_str(&format!("{}:{}\n", account.username, cookie));
                }
            }
        }
        "json" => {
            let mut exported = Vec::new();
            for account in &accounts {
                if let Ok(cookie) = store.get_cookie(&account.id) {
                    exported.push(ExportedAccount {
                        username: account.username.clone(),
                        cookie,
                        group: account.group.clone(),
                        alias: account.alias.clone(),
                    });
                }
            }
            output = serde_json::to_string_pretty(&exported)
                .map_err(|e| AppError::Other(format!("JSON serialization error: {}", e)))?;
        }
        _ => {
            return Err(AppError::Other(format!(
                "Unknown export format: {}. Use 'cookies', 'user:cookie', or 'json'",
                format
            )));
        }
    }

    Ok(output)
}

/// Get the cookie for an account (for copy-to-clipboard)
#[tauri::command]
pub async fn get_account_cookie(
    account_id: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let uuid = uuid::Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let store = state.store.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;

    store.get_cookie(&uuid)
}

/// Update the cookie for an existing account
#[tauri::command]
pub async fn update_account_cookie(
    account_id: String,
    new_cookie: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let uuid = uuid::Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    // Validate the new cookie
    state.roblox.get_authenticated_user(&new_cookie).await?;

    let mut store = state.store.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;

    store.update_cookie(&uuid, &new_cookie)
}
