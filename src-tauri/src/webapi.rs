use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State as AxumState};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::AccountView;
use crate::storage::AccountStore;

/// Shared state for the web API — wraps the same Mutex<AccountStore> as Tauri
type SharedStore = Arc<Mutex<AccountStore>>;

/// API error response
#[derive(Serialize)]
struct ApiError {
    error: String,
}

/// API success response wrapper
#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: T,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data,
        })
    }
}

fn api_error(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (
        status,
        Json(ApiError {
            error: msg.into(),
        }),
    )
}

/// Query params for listing accounts
#[derive(Deserialize)]
struct ListQuery {
    group: Option<String>,
}

/// Start the local Web API server on the given port
pub async fn start_web_api(store: SharedStore, port: u16) {
    let app = Router::new()
        .route("/accounts", get(list_accounts))
        .route("/accounts/count", get(get_count))
        .route("/accounts/{id}", get(get_account))
        .route("/accounts/{id}/cookie", get(get_cookie))
        .route("/groups", get(get_groups))
        .route("/health", get(health))
        .with_state(store);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    log::info!("Web API server starting on http://{}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("Failed to bind Web API server on port {}: {}", port, e);
            return;
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        log::error!("Web API server error: {}", e);
    }
}

/// GET /health
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// GET /accounts?group=optional
async fn list_accounts(
    AxumState(store): AxumState<SharedStore>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let store = store.lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Lock poisoned"))?;

    let mut accounts: Vec<AccountView> = store
        .list_accounts()
        .iter()
        .map(|a| a.to_view())
        .collect();

    if let Some(group) = query.group {
        accounts.retain(|a| a.group.as_deref() == Some(&group));
    }

    Ok(ApiResponse::ok(accounts))
}

/// GET /accounts/:id
async fn get_account(
    AxumState(store): AxumState<SharedStore>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "Invalid UUID"))?;

    let store = store.lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Lock poisoned"))?;

    let account = store
        .get_account(&uuid)
        .map_err(|_| api_error(StatusCode::NOT_FOUND, "Account not found"))?;

    Ok(ApiResponse::ok(account.to_view()))
}

/// GET /accounts/:id/cookie
async fn get_cookie(
    AxumState(store): AxumState<SharedStore>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "Invalid UUID"))?;

    let store = store.lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Lock poisoned"))?;

    let cookie = store
        .get_cookie(&uuid)
        .map_err(|_| api_error(StatusCode::NOT_FOUND, "Account not found"))?;

    Ok(ApiResponse::ok(serde_json::json!({ "cookie": cookie })))
}

/// GET /accounts/count
async fn get_count(
    AxumState(store): AxumState<SharedStore>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let store = store.lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Lock poisoned"))?;

    Ok(ApiResponse::ok(serde_json::json!({ "count": store.count() })))
}

/// GET /groups
async fn get_groups(
    AxumState(store): AxumState<SharedStore>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let store = store.lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Lock poisoned"))?;

    Ok(ApiResponse::ok(store.get_groups()))
}
