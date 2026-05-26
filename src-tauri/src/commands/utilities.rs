use tauri::State;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::roblox::{BirthdateResponse, PrivacySettings};
use crate::state::AppState;

/// Change password for an account
#[tauri::command]
pub async fn change_password(
    account_id: String,
    old_password: String,
    new_password: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    state
        .roblox
        .change_password(&cookie, &old_password, &new_password)
        .await
}

/// Change email for an account
#[tauri::command]
pub async fn change_email(
    account_id: String,
    password: String,
    new_email: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    state
        .roblox
        .change_email(&cookie, &password, &new_email)
        .await
}

/// Get privacy settings for an account
#[tauri::command]
pub async fn get_privacy_settings(
    account_id: String,
    state: State<'_, AppState>,
) -> AppResult<PrivacySettings> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    state.roblox.get_privacy_settings(&cookie).await
}

/// Set description for an account
#[tauri::command]
pub async fn set_account_description(
    account_id: String,
    description: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    state
        .roblox
        .set_description(&cookie, &description)
        .await?;

    // Also update local store
    {
        let mut store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        let mut account = store.get_account(&uuid)?.clone();
        account.description = if description.is_empty() {
            None
        } else {
            Some(description)
        };
        store.update_account(account)?;
    }

    Ok(())
}

/// Set display name for an account
#[tauri::command]
pub async fn set_display_name(
    account_id: String,
    new_display_name: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let (cookie, user_id) = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        let cookie = store.get_cookie(&uuid)?;
        let account = store.get_account(&uuid)?;
        let user_id = account
            .user_id
            .ok_or_else(|| AppError::Other("Account has no user ID".into()))?;
        (cookie, user_id)
    };

    state
        .roblox
        .set_display_name(&cookie, user_id, &new_display_name)
        .await?;

    // Update local store
    {
        let mut store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        let mut account = store.get_account(&uuid)?.clone();
        account.display_name = Some(new_display_name);
        store.update_account(account)?;
    }

    Ok(())
}

/// Get gender for an account
#[tauri::command]
pub async fn get_gender(
    account_id: String,
    state: State<'_, AppState>,
) -> AppResult<u8> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    state.roblox.get_gender(&cookie).await
}

/// Get birthdate for an account
#[tauri::command]
pub async fn get_birthdate(
    account_id: String,
    state: State<'_, AppState>,
) -> AppResult<BirthdateResponse> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let cookie = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        store.get_cookie(&uuid)?
    };

    state.roblox.get_birthdate(&cookie).await
}
