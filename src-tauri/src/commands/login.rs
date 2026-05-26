use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{Account, AccountView};
use crate::state::AppState;

#[derive(Clone, Serialize)]
pub struct LoginCompletePayload {
    pub success: bool,
    pub account: Option<AccountView>,
    pub error: Option<String>,
}

/// Open a WebView window to roblox.com/login, intercept the .ROBLOSECURITY cookie
/// after successful login, then add the account automatically.
#[tauri::command]
pub async fn open_login_window(app: AppHandle) -> AppResult<()> {
    // Close existing login window if any
    if let Some(existing) = app.get_webview_window("roblox-login") {
        let _ = existing.close();
    }

    let app_handle = app.clone();
    // Prevent duplicate extraction if multiple page loads fire
    let extracted = Arc::new(AtomicBool::new(false));

    // Build the login window with on_page_load callback
    let _login_window = WebviewWindowBuilder::new(
        &app,
        "roblox-login",
        WebviewUrl::External("https://www.roblox.com/login".parse().unwrap()),
    )
    .title("Roblox Login")
    .inner_size(900.0, 700.0)
    .center()
    .resizable(true)
    .on_page_load(move |window, payload| {
        if payload.event() == PageLoadEvent::Finished {
            let url_str = payload.url().to_string();
            log::info!("Login page loaded: {}", url_str);

            // After login, Roblox redirects to /home or /id/home or similar
            let is_home = url_str.contains("/home")
                && url_str.contains("roblox.com")
                && !url_str.contains("/login");
            if is_home {
                // Only extract once
                if extracted.swap(true, Ordering::SeqCst) {
                    return;
                }

                let window_clone = window.clone();
                let handle = app_handle.clone();

                tauri::async_runtime::spawn(async move {
                    // Wait for cookies to be fully set
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    extract_cookie_and_add_account(handle, window_clone).await;
                });
            }
        }
    })
    .build()
    .map_err(|e| AppError::Other(format!("Failed to create login window: {}", e)))?;

    // Clear all browsing data so the login page is fresh (no stale sessions)
    let _ = _login_window.clear_all_browsing_data();
    let _ = _login_window.navigate("https://www.roblox.com/login".parse().unwrap());

    Ok(())
}

/// Extract the .ROBLOSECURITY cookie from the login WebView and add the account
async fn extract_cookie_and_add_account(
    app: AppHandle,
    login_window: tauri::WebviewWindow,
) {
    let result = do_extract_and_add(&app, &login_window).await;

    let payload = match result {
        Ok(view) => LoginCompletePayload {
            success: true,
            account: Some(view),
            error: None,
        },
        Err(e) => LoginCompletePayload {
            success: false,
            account: None,
            error: Some(e.to_string()),
        },
    };

    // Emit event to main window
    let _ = app.emit_to("main", "login-complete", payload);

    // Close the login window
    let _ = login_window.close();
}

async fn do_extract_and_add(
    app: &AppHandle,
    login_window: &tauri::WebviewWindow,
) -> AppResult<AccountView> {
    // Try cookies_for_url first, then fall back to all cookies
    let roblox_url: tauri::Url = "https://www.roblox.com".parse().unwrap();
    let mut cookies = login_window
        .cookies_for_url(roblox_url)
        .map_err(|e| AppError::Other(format!("Failed to get cookies for URL: {}", e)))?;

    log::info!("cookies_for_url returned {} cookies", cookies.len());
    for c in &cookies {
        log::info!("  cookie: {} = {}...", c.name(), &c.value().chars().take(20).collect::<String>());
    }

    // If cookies_for_url didn't find it, try all cookies
    if !cookies.iter().any(|c| c.name() == ".ROBLOSECURITY") {
        log::info!("Trying cookies() fallback...");
        cookies = login_window
            .cookies()
            .map_err(|e| AppError::Other(format!("Failed to get all cookies: {}", e)))?;
        log::info!("cookies() returned {} cookies", cookies.len());
        for c in &cookies {
            if c.name() == ".ROBLOSECURITY" {
                log::info!("  Found .ROBLOSECURITY in all cookies!");
            }
        }
    }

    // Find the .ROBLOSECURITY cookie
    let roblosecurity = cookies
        .iter()
        .find(|c| c.name() == ".ROBLOSECURITY")
        .ok_or_else(|| {
            AppError::InvalidCookie("No .ROBLOSECURITY cookie found after login. Try logging in again.".into())
        })?;

    let cookie_value = roblosecurity.value().to_string();

    if cookie_value.is_empty() {
        return Err(AppError::InvalidCookie(
            ".ROBLOSECURITY cookie is empty".into(),
        ));
    }

    // Use the cookie to fetch user info and add the account
    let state = app.state::<AppState>();

    // Validate cookie by fetching user info
    let user = state.roblox.get_authenticated_user(&cookie_value).await?;

    let mut account = Account::new(user.name.clone());
    account.user_id = Some(user.id);
    account.display_name = Some(user.display_name);

    // Fetch additional info in parallel
    let (robux, premium, avatar_url, details) = tokio::join!(
        state.roblox.get_robux(user.id, &cookie_value),
        state.roblox.is_premium(user.id, &cookie_value),
        state.roblox.get_avatar_url(user.id),
        state.roblox.get_user_details(user.id, &cookie_value),
    );

    account.robux = robux.ok();
    account.is_premium = premium.ok();
    account.avatar_url = avatar_url.ok().flatten();
    if let Ok(details) = details {
        account.description = details.description;
    }

    let view = account.to_view();

    let mut store = state.store.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;
    store.add_account(account, &cookie_value)?;

    log::info!("Account added via WebView login: {}", view.username);

    Ok(view)
}

/// Open a browser window logged in as the specified account.
/// Creates a WebView2 window, navigates to roblox.com, injects the .ROBLOSECURITY cookie
/// via JavaScript, then reloads so the session is authenticated.
#[tauri::command]
pub async fn open_browser(
    account_id: String,
    url: Option<String>,
    app: AppHandle,
) -> AppResult<()> {
    let uuid = Uuid::parse_str(&account_id)
        .map_err(|e| AppError::Other(format!("Invalid UUID: {}", e)))?;

    let state = app.state::<AppState>();
    let (cookie, username) = {
        let store = state.store.lock().map_err(|e| {
            AppError::Other(format!("Lock poisoned: {}", e))
        })?;
        let account = store.get_account(&uuid)?;
        let cookie = store.get_cookie(&uuid)?;
        (cookie, account.username.clone())
    };

    // Close existing browser window for this account if any
    let window_label = format!("browser-{}", &account_id[..8]);
    if let Some(existing) = app.get_webview_window(&window_label) {
        let _ = existing.close();
    }

    let target_url = url.unwrap_or_else(|| "https://www.roblox.com/home".to_string());
    let cookie_value = cookie.trim_start_matches(".ROBLOSECURITY=").to_string();

    // Build browser window starting at about:blank
    let browser_window = WebviewWindowBuilder::new(
        &app,
        &window_label,
        WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title(format!("Roblox - {}", username))
    .inner_size(1200.0, 800.0)
    .center()
    .resizable(true)
    .on_page_load(build_browser_page_handler(cookie_value.clone(), target_url.clone()))
    .build()
    .map_err(|e| AppError::Other(format!("Failed to create browser window: {}", e)))?;

    // Clear browsing data first, then use on_page_load to inject cookie via JS
    let _ = browser_window.clear_all_browsing_data();

    // Wait then navigate — on_page_load will handle cookie injection
    let win = browser_window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        let _ = win.navigate("https://www.roblox.com/login".parse().unwrap());
    });

    Ok(())
}

/// Build the on_page_load handler that injects cookie and address bar
fn build_browser_page_handler(
    cookie_value: String,
    target_url: String,
) -> impl Fn(tauri::WebviewWindow, tauri::webview::PageLoadPayload<'_>) + Send + Sync + 'static {
    let cookie_injected = Arc::new(AtomicBool::new(false));

    move |window, payload| {
        if payload.event() != PageLoadEvent::Finished {
            return;
        }
        let url_str = payload.url().to_string();

        // Step 1: Once on roblox.com, inject cookie and redirect to target
        if url_str.contains("roblox.com") && !url_str.contains("about:blank") 
            && !cookie_injected.swap(true, Ordering::SeqCst) 
        {
            let escaped = cookie_value
                .replace('\\', "\\\\")
                .replace('\'', "\\'")
                .replace('"', "\\\"");
            // Set cookie via JS (non-httpOnly but Roblox accepts it for web)
            // Then delete any existing httpOnly version by expiring it, and set fresh
            let js = format!(
                r#"
                document.cookie = ".ROBLOSECURITY=expired; domain=.roblox.com; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
                document.cookie = ".ROBLOSECURITY={cookie}; domain=.roblox.com; path=/; secure; max-age=31536000";
                setTimeout(function() {{ window.location.href = "{url}"; }}, 200);
                "#,
                cookie = escaped,
                url = target_url,
            );
            let _ = window.eval(&js);
            return;
        }

        // Step 2: Inject address bar on subsequent pages
        if !url_str.contains("roblox.com") || url_str.contains("/login") {
            return;
        }

        let address_bar_js = r#"
        (function() {
            if (document.getElementById('ram-address-bar')) return;
            var bar = document.createElement('div');
            bar.id = 'ram-address-bar';
            bar.style.cssText = 'display:flex;align-items:center;gap:6px;padding:4px 8px;background:#1e1e2d;border-bottom:1px solid #3f3f4e;font-family:system-ui;position:sticky;top:0;z-index:999999;';
            var back = document.createElement('button');
            back.textContent = '\u{2190}';
            back.style.cssText = 'background:#2a2a3c;color:#fff;border:none;border-radius:4px;padding:4px 10px;cursor:pointer;font-size:16px;';
            back.onclick = function() { history.back(); };
            var fwd = document.createElement('button');
            fwd.textContent = '\u{2192}';
            fwd.style.cssText = back.style.cssText;
            fwd.onclick = function() { history.forward(); };
            var reload = document.createElement('button');
            reload.textContent = '\u{21BB}';
            reload.style.cssText = back.style.cssText;
            reload.onclick = function() { location.reload(); };
            var input = document.createElement('input');
            input.type = 'text';
            input.value = location.href;
            input.style.cssText = 'flex:1;background:#12121c;color:#fff;border:1px solid #3f3f4e;border-radius:4px;padding:5px 10px;font-size:13px;outline:none;';
            input.addEventListener('focus', function() { this.select(); });
            input.addEventListener('keydown', function(e) {
                if (e.key === 'Enter') {
                    var url = this.value.trim();
                    if (!url.startsWith('http')) url = 'https://' + url;
                    location.href = url;
                }
            });
            bar.appendChild(back);
            bar.appendChild(fwd);
            bar.appendChild(reload);
            bar.appendChild(input);
            document.body.prepend(bar);
            // Inject style that shifts everything down
            var style = document.createElement('style');
            style.id = 'ram-bar-style';
            style.textContent = 'html { margin-top: 40px !important; height: calc(100% - 40px) !important; } #ram-address-bar { position: fixed; top: 0; left: 0; right: 0; height: 40px; margin-top: -40px; z-index: 2147483647; box-sizing: border-box; }';
            document.head.appendChild(style);
        })();
        "#;
        let _ = window.eval(address_bar_js);
    }
}
