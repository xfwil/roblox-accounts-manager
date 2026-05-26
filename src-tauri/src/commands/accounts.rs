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

/// Refresh account info from Roblox API
#[tauri::command]
pub async fn refresh_account(id: String, state: State<'_, AppState>) -> AppResult<AccountView> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| crate::error::AppError::Other(format!("Invalid UUID: {}", e)))?;

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            crate::error::AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    let user = state.roblox.get_authenticated_user(&cookie).await?;

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
