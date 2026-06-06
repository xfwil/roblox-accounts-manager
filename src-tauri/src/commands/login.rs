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
/// Each account gets an isolated data_directory so cookies/sessions don't leak between accounts.
/// Uses initialization_script to inject cookie + address bar on every page load.
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
        // Brief pause so WebView2 releases the data directory lock
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    // Per-account isolated data directory — each account has its own cookie store
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| AppError::Other(format!("Failed to get app data dir: {}", e)))?;
    let browser_data_dir = app_data_dir.join("browser-sessions").join(&account_id[..8]);
    std::fs::create_dir_all(&browser_data_dir)
        .map_err(|e| AppError::Other(format!("Failed to create browser data dir: {}", e)))?;

    let target_url = url.unwrap_or_else(|| "https://www.roblox.com/home".to_string());
    let cookie_value = cookie.trim_start_matches(".ROBLOSECURITY=").to_string();

    let escaped_cookie = cookie_value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"");

    // Address bar script injected on EVERY page load via initialization_script.
    // Also intercepts window.open() and target=_blank links since WebView2 is single-tab.
    let address_bar_script = r#"
    (function() {
        'use strict';
        if (window.__RAM_BAR_INIT__) return;
        window.__RAM_BAR_INIT__ = true;

        // === Intercept window.open / target=_blank ===
        // WebView2 doesn't support multiple tabs, so redirect new-tab requests
        // to navigate in the current window instead.
        var _origOpen = window.open;
        window.open = function(url, target, features) {
            if (url && /^https?:/i.test(String(url))) {
                window.location.href = String(url);
                return window;
            }
            // For non-http (javascript:, blob:, etc.) or empty, use original
            return _origOpen ? _origOpen.apply(this, arguments) : null;
        };

        // Intercept clicks on <a target="_blank"> links
        document.addEventListener('click', function(e) {
            var link = e.target.closest ? e.target.closest('a[target="_blank"]') : null;
            if (!link) {
                // Also check parent elements manually for older DOM
                var el = e.target;
                while (el && el !== document.body) {
                    if (el.tagName === 'A' && el.target && el.target.toLowerCase() === '_blank') {
                        link = el;
                        break;
                    }
                    el = el.parentElement;
                }
            }
            if (link && link.href && /^https?:/i.test(link.href)) {
                e.preventDefault();
                e.stopPropagation();
                window.location.href = link.href;
            }
        }, true); // capture phase to intercept before page handlers

        // === Address bar UI ===
        function injectBar() {
            if (document.getElementById('ram-nav-bar')) return;
            if (!document.body) return;

            var bar = document.createElement('div');
            bar.id = 'ram-nav-bar';

            var back = document.createElement('button');
            back.className = 'ram-btn';
            back.textContent = '\u{2190}';
            back.title = 'Back';
            back.onclick = function() { history.back(); };

            var fwd = document.createElement('button');
            fwd.className = 'ram-btn';
            fwd.textContent = '\u{2192}';
            fwd.title = 'Forward';
            fwd.onclick = function() { history.forward(); };

            var reload = document.createElement('button');
            reload.className = 'ram-btn';
            reload.textContent = '\u{21BB}';
            reload.title = 'Reload';
            reload.onclick = function() { location.reload(); };

            var input = document.createElement('input');
            input.id = 'ram-url-input';
            input.type = 'text';
            input.value = location.href;
            input.spellcheck = false;
            input.addEventListener('focus', function() { this.select(); });
            input.addEventListener('keydown', function(e) {
                if (e.key === 'Enter') {
                    var url = this.value.trim();
                    if (url && !url.startsWith('http')) url = 'https://' + url;
                    if (url) location.href = url;
                }
            });

            bar.appendChild(back);
            bar.appendChild(fwd);
            bar.appendChild(reload);
            bar.appendChild(input);

            // Inject styles
            var style = document.createElement('style');
            style.id = 'ram-bar-style';
            style.textContent = [
                '#ram-nav-bar {',
                '  position: fixed; top: 0; left: 0; right: 0; height: 42px;',
                '  display: flex; align-items: center; gap: 6px;',
                '  padding: 0 10px;',
                '  background: #1a1a2e; border-bottom: 1px solid #30304a;',
                '  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;',
                '  z-index: 2147483647;',
                '  box-sizing: border-box;',
                '  user-select: none;',
                '}',
                '#ram-nav-bar .ram-btn {',
                '  background: #2a2a40; color: #e0e0e0; border: 1px solid #3a3a55;',
                '  border-radius: 6px; padding: 6px 12px; cursor: pointer;',
                '  font-size: 15px; line-height: 1; transition: background 0.15s;',
                '  min-width: 34px; text-align: center;',
                '}',
                '#ram-nav-bar .ram-btn:hover { background: #3a3a58; }',
                '#ram-nav-bar .ram-btn:active { background: #4a4a68; }',
                '#ram-url-input {',
                '  flex: 1; background: #0f0f1a; color: #f0f0f0;',
                '  border: 1px solid #3a3a55; border-radius: 6px;',
                '  padding: 7px 12px; font-size: 13px; outline: none;',
                '  min-width: 0;',
                '}',
                '#ram-url-input:focus { border-color: #5865f2; }',
                'body { margin-top: 42px !important; }',
                'html { overflow-x: hidden; }',
            ].join('\n');

            document.head.appendChild(style);
            document.body.prepend(bar);
        }

        // Update URL in the bar on navigation (SPA support)
        function updateUrl() {
            var input = document.getElementById('ram-url-input');
            if (input && input !== document.activeElement) {
                input.value = location.href;
            }
        }

        // Inject as soon as DOM is ready
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', injectBar);
        } else {
            injectBar();
        }

        // Also try after a short delay as fallback
        setTimeout(injectBar, 100);
        setTimeout(injectBar, 500);

        // Listen for SPA navigation changes
        var _pushState = history.pushState;
        history.pushState = function() {
            _pushState.apply(this, arguments);
            setTimeout(updateUrl, 50);
        };
        var _replaceState = history.replaceState;
        history.replaceState = function() {
            _replaceState.apply(this, arguments);
            setTimeout(updateUrl, 50);
        };
        window.addEventListener('popstate', function() { setTimeout(updateUrl, 50); });
    })();
    "#;

    // Cookie + redirect script: runs at document_start on every navigation.
    // Sets cookie immediately, then if on /login page, redirects to target without waiting.
    let cookie_redirect_script = format!(
        r#"
        (function() {{
            'use strict';
            if (!location.hostname.includes('roblox.com')) return;
            // Always set cookie at document_start (before page JS reads it)
            document.cookie = ".ROBLOSECURITY=expired; domain=.roblox.com; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
            document.cookie = ".ROBLOSECURITY={cookie}; domain=.roblox.com; path=/; secure; max-age=31536000";
            // If we landed on login/challenge page, redirect to target immediately
            if (!window.__RAM_REDIRECTED__ && (location.pathname.startsWith('/login') || location.pathname.startsWith('/Login'))) {{
                window.__RAM_REDIRECTED__ = true;
                window.location.replace("{url}");
            }}
        }})();
        "#,
        cookie = escaped_cookie,
        url = target_url,
    );

    // Chrome-like user agent so Roblox serves the full desktop site
    let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

    // Build browser window — navigates directly to target URL
    // initialization_script fires at document_start (before any page JS),
    // so cookie is set before Roblox reads it. data_directory isolates per account.
    let _browser_window = WebviewWindowBuilder::new(
        &app,
        &window_label,
        WebviewUrl::External("https://www.roblox.com/login".parse().unwrap()),
    )
    .title(format!("Roblox - {}", username))
    .inner_size(1280.0, 860.0)
    .min_inner_size(800.0, 500.0)
    .center()
    .resizable(true)
    .user_agent(user_agent)
    .data_directory(browser_data_dir)
    .initialization_script(&cookie_redirect_script)
    .initialization_script(address_bar_script)
    .build()
    .map_err(|e| AppError::Other(format!("Failed to create browser window: {}", e)))?;

    Ok(())
}

