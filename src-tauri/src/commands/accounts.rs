use tauri::State;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{Account, AccountView};
use crate::state::AppState;

/// List all accounts (returns views without secrets)
#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> AppResult<Vec<AccountView>> {
    let store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    let accounts = store.list_accounts();
    Ok(accounts.iter().map(|a| a.to_view()).collect())
}

/// Get a single account by ID
#[tauri::command]
pub async fn get_account(id: String, state: State<'_, AppState>) -> AppResult<AccountView> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| crate::error::AppError::Other(format!("Invalid UUID: {}", e)))?;
    let store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    let account = store.get_account(&uuid)?;
    Ok(account.to_view())
}

/// Add a new account with a cookie.
/// If an account with the same user_id already exists, update its cookie and data.
#[tauri::command]
pub async fn add_account(
    cookie: String,
    group: Option<String>,
    alias: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<AccountView> {
    // Validate cookie by fetching user info
    let user = state.roblox.get_authenticated_user(&cookie).await?;

    // Fetch additional info in parallel
    let (robux, premium, avatar_url, details) = tokio::join!(
        state.roblox.get_robux(user.id, &cookie),
        state.roblox.is_premium(user.id, &cookie),
        state.roblox.get_avatar_url(user.id),
        state.roblox.get_user_details(user.id, &cookie),
    );

    let mut store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;

    // Check if account with same user_id already exists
    let existing = store.list_accounts().iter()
        .find(|a| a.user_id == Some(user.id))
        .map(|a| a.id);

    if let Some(existing_id) = existing {
        // Update existing account's cookie and data
        let mut account = store.get_account(&existing_id)?.clone();
        account.username = user.name;
        account.display_name = Some(user.display_name);
        account.robux = robux.ok();
        account.is_premium = premium.ok();
        account.avatar_url = avatar_url.ok().flatten();
        if let Ok(details) = details {
            account.description = details.description;
        }
        account.last_used = Some(chrono::Utc::now());
        // Only update group/alias if provided
        if group.is_some() { account.group = group; }
        if alias.is_some() { account.alias = alias; }

        store.update_cookie(&existing_id, &cookie)?;
        let view = account.to_view();
        store.update_account(account)?;
        Ok(view)
    } else {
        // Create new account
        let mut account = Account::new(user.name.clone());
        account.user_id = Some(user.id);
        account.display_name = Some(user.display_name);
        account.group = group;
        account.alias = alias;
        account.robux = robux.ok();
        account.is_premium = premium.ok();
        account.avatar_url = avatar_url.ok().flatten();
        if let Ok(details) = details {
            account.description = details.description;
        }

        let view = account.to_view();
        store.add_account(account, &cookie)?;
        Ok(view)
    }
}

/// Remove an account
#[tauri::command]
pub async fn remove_account(id: String, state: State<'_, AppState>) -> AppResult<()> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| crate::error::AppError::Other(format!("Invalid UUID: {}", e)))?;
    let mut store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    store.remove_account(&uuid)?;
    Ok(())
}

/// Update account metadata (alias, group, fields, etc.)
#[tauri::command]
pub async fn update_account(
    id: String,
    alias: Option<String>,
    group: Option<String>,
    description: Option<String>,
    fields: Option<std::collections::HashMap<String, String>>,
    state: State<'_, AppState>,
) -> AppResult<AccountView> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| crate::error::AppError::Other(format!("Invalid UUID: {}", e)))?;

    let mut store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;

    let mut account = store.get_account(&uuid)?.clone();

    if let Some(alias) = alias {
        account.alias = if alias.is_empty() { None } else { Some(alias) };
    }
    if let Some(group) = group {
        account.group = if group.is_empty() { None } else { Some(group) };
    }
    if let Some(desc) = description {
        account.description = if desc.is_empty() { None } else { Some(desc) };
    }
    if let Some(fields) = fields {
        account.fields = fields;
    }

    let view = account.to_view();
    store.update_account(account)?;

    Ok(view)
}

/// Manual "Check unlock": presses Unlock via the API and reports which
/// challenge Roblox asks for (captcha / biometric), or that it unlocked.
#[tauri::command]
pub async fn check_unlock(id: String, app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| crate::error::AppError::Other(format!("Invalid UUID: {}", e)))?;
    let (cookie, username) = {
        let store = state.store.lock().map_err(|e| crate::error::AppError::Other(format!("Lock poisoned: {}", e)))?;
        (store.get_cookie(&uuid)?, store.get_account(&uuid)?.username.clone())
    };
    let (status, challenge, body) = state.roblox.press_unlock(&cookie).await?;
    log::info!("[{username}] check unlock: status={status} challenge={challenge:?} body={}", body.chars().take(300).collect::<String>());
    if status == 401 {
        if let Ok(mut store) = state.store.lock() {
            let _ = store.set_flag(&uuid, "cookie_dead", true);
        }
        return Err(crate::error::AppError::InvalidCookie("Cookie is invalid or expired".into()));
    }
    let message = crate::commands::apply_unlock_result(&app, uuid, challenge.as_deref(), status, true)
        .unwrap_or_else(|| format!("{username}: no challenge (HTTP {status}). {}", body.chars().take(160).collect::<String>()));
    use tauri::Emitter;
    let _ = app.emit("browser-cookie", serde_json::json!({ "saved": true, "message": message }));
    Ok(message)
}

/// Refresh account info from Roblox API
#[tauri::command]
pub async fn refresh_account(id: String, state: State<'_, AppState>) -> AppResult<AccountView> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| crate::error::AppError::Other(format!("Invalid UUID: {}", e)))?;

    let (cookie, known_id, known_name) = {
        let store = state.store.lock().map_err(|e| {
            crate::error::AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        let acc = store.get_account(&uuid)?;
        (store.get_cookie(&uuid)?, acc.user_id, acc.username.clone())
    };

    let user = match state.roblox.get_authenticated_user(&cookie).await {
        Ok(u) => u,
        Err(e) => {
            // Moderated: ask the public profile whether it's a permanent ban.
            // Unknown (lookup failed) leaves the banned flag as it was.
            let banned = match &e {
                crate::error::AppError::Moderated(_) => match state.roblox.is_banned(known_id, &known_name).await {
                    Ok(b) => {
                        log::info!("[{known_name}] moderated, isBanned={b}");
                        Some(b)
                    }
                    Err(err) => {
                        log::warn!("[{known_name}] ban check failed: {err}");
                        None
                    }
                },
                _ => None,
            };
            // Persist durable account states; network/5xx blips flag nothing.
            //  - 401 InvalidCookie  → cookie is dead (re-login needed)
            //  - 403 Moderated      → cookie alive but account is banned/moderated
            if let Ok(mut store) = state.store.lock() {
                match &e {
                    crate::error::AppError::InvalidCookie(_) => {
                        let _ = store.set_flag(&uuid, "cookie_dead", true);
                        let _ = store.set_flag(&uuid, "moderated", false);
                    }
                    crate::error::AppError::Moderated(_) => {
                        let _ = store.set_flag(&uuid, "moderated", true);
                        let _ = store.set_flag(&uuid, "cookie_dead", false);
                        if let Some(b) = banned {
                            let _ = store.set_flag(&uuid, "banned", b);
                        }
                    }
                    _ => {}
                }
            }
            return Err(e);
        }
    };

    let (robux, premium, avatar_url, details) = tokio::join!(
        state.roblox.get_robux(user.id, &cookie),
        state.roblox.is_premium(user.id, &cookie),
        state.roblox.get_avatar_url(user.id),
        state.roblox.get_user_details(user.id, &cookie),
    );

    let mut store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;

    let mut account = store.get_account(&uuid)?.clone();
    account.username = user.name;
    account.user_id = Some(user.id);
    account.display_name = Some(user.display_name);
    account.robux = robux.ok();
    account.is_premium = premium.ok();
    account.avatar_url = avatar_url.ok().flatten();
    if let Ok(details) = details {
        account.description = details.description;
    }
    // Refresh succeeded → the cookie is alive and the account is in good
    // standing; clear any stale problem markers. Face-lock is only a login-time
    // gate marker — a clean authenticated refresh means it's been unlocked, so
    // drop it too (a future locked login would re-flag it).
    account.fields.remove("cookie_dead");
    account.fields.remove("moderated");
    account.fields.remove("banned");
    account.fields.remove("unlock_challenge");
    account.fields.remove("facelock");

    let view = account.to_view();
    store.update_account(account)?;

    Ok(view)
}

/// Reorder accounts
#[tauri::command]
pub async fn reorder_accounts(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let uuids: Result<Vec<Uuid>, _> = ids.iter().map(|id| Uuid::parse_str(id)).collect();
    let uuids = uuids.map_err(|e| crate::error::AppError::Other(format!("Invalid UUID: {}", e)))?;

    let mut store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    store.reorder(&uuids)?;
    Ok(())
}

/// Get all group names
#[tauri::command]
pub async fn get_groups(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    Ok(store.get_groups())
}

/// Get account count
#[tauri::command]
pub async fn get_account_count(state: State<'_, AppState>) -> AppResult<usize> {
    let store = state.store.lock().map_err(|e| {
        crate::error::AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    Ok(store.count())
}
