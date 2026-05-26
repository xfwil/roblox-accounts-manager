use tauri::State;

use crate::error::{AppError, AppResult};
use crate::roblox::UserPresence;
use crate::state::AppState;

/// Get presence status for all accounts that have user_ids
/// Uses the first account's cookie for authentication
#[tauri::command]
pub async fn get_account_presences(
    state: State<'_, AppState>,
) -> AppResult<Vec<UserPresence>> {
    let (user_ids, first_cookie) = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;

        let accounts = store.list_accounts();
        let user_ids: Vec<u64> = accounts
            .iter()
            .filter_map(|a| a.user_id)
            .collect();

        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Use the first account's cookie for the presence API call
        let first_account_with_cookie = accounts
            .iter()
            .find(|a| a.user_id.is_some());

        let cookie = match first_account_with_cookie {
            Some(account) => store.get_cookie(&account.id).ok(),
            None => None,
        };

        (user_ids, cookie)
    };

    let cookie = first_cookie.ok_or_else(|| {
        AppError::Other("No account cookie available for presence check".into())
    })?;

    state.roblox.get_user_presences(&cookie, &user_ids).await
}

/// Get presence for a specific account
#[tauri::command]
pub async fn get_single_presence(
    account_id: String,
    state: State<'_, AppState>,
) -> AppResult<UserPresence> {
    let uuid = uuid::Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let (user_id, cookie) = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;

        let account = store.get_account(&uuid)?;
        let user_id = account.user_id.ok_or_else(|| {
            AppError::Other("Account has no user_id".into())
        })?;
        let cookie = store.get_cookie(&uuid)?;
        (user_id, cookie)
    };

    let presences = state.roblox.get_user_presences(&cookie, &[user_id]).await?;

    presences.into_iter().next().ok_or_else(|| {
        AppError::Other("No presence data returned".into())
    })
}
