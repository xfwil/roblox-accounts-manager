use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
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

#[derive(Clone, Serialize)]
pub struct LoginStatePayload {
    /// Window label the state belongs to (e.g. "roblox-login" or "bulk-login-0").
    pub label: String,
    /// One of: login, captcha, locked, twostep, error, success, loading.
    pub state: String,
}

/// State codes the detect script can report. Any window title whose first
/// " | "-segment isn't one of these is the page's own title — ignored.
const KNOWN_STATES: [&str; 7] = [
    "login", "captcha", "locked", "twostep", "error", "success", "loading",
];

/// Injected at document-start on every roblox.com login page. Classifies the
/// page into a known state and encodes it into document.title as
/// `"<code> | <human text>"`. Rust reads it back via window.title() — a
/// capability-free side channel that works on external (roblox.com) origins.
/// ponytail: title side-channel, ~900ms poll can miss a flicker; move to a
/// real IPC command + per-window capability if detection proves flaky.
const DETECT_JS: &str = r#"
(function () {
  'use strict';
  if (window.top !== window.self) return;      // top frame only (skip arkose iframe)
  if (window.__RAM_DETECT__) return;
  window.__RAM_DETECT__ = true;

  function classify() {
    var href = location.href;
    if (/\/home/.test(href) && !/\/login/.test(href)) return ['success', 'Signed in'];
    if (document.querySelector('iframe[src*="arkoselabs"],iframe[src*="funcaptcha"],iframe[title*="challenge" i],[id^="arkose"],#arkose-game,#FunCaptcha')) return ['captcha', 'Solve the challenge to continue'];
    var txt = document.body ? document.body.innerText : '';
    if (/account\s+(?:has been\s+)?locked|suspicious activity|confirm(?:ing)?\s+that\s+you['’]?re\s+a?\s*human|verify\s+(?:that\s+)?you['’]?re\s+a?\s*human/i.test(txt)) return ['locked', 'Account locked — confirm you are human'];
    if (/two[- ]step|2-step|verification code|security code|authenticator/i.test(txt) || /2sv|two-step/i.test(href)) return ['twostep', 'Enter your verification code'];
    if (/incorrect username or password|wasn['’]?t quite right|didn['’]?t match|invalid password/i.test(txt)) return ['error', 'Incorrect username or password'];
    if (document.querySelector('#login-username, input#login-username, form#login-form')) return ['login', 'Waiting for login'];
    return ['loading', 'Loading…'];
  }

  function tick() {
    try {
      var s = classify();
      var marker = s[0] + ' | ' + s[1];
      if (document.title !== marker) document.title = marker;
    } catch (e) {}
  }

  tick();
  setInterval(tick, 900);
  if (document.body) new MutationObserver(tick).observe(document.body, { childList: true, subtree: true });
  else document.addEventListener('DOMContentLoaded', function () {
    tick();
    new MutationObserver(tick).observe(document.body, { childList: true, subtree: true });
  });
})();
"#;

/// Is a non-empty `.ROBLOSECURITY` cookie present in this window right now?
/// Cheap/local (no network). The cookie is the prize: once it exists we can
/// harvest it even behind a face-lock / captcha interstitial.
fn cookie_present(win: &tauri::WebviewWindow) -> bool {
    let url: tauri::Url = "https://www.roblox.com".parse().unwrap();
    if let Ok(cs) = win.cookies_for_url(url) {
        if cs.iter().any(|c| c.name() == ".ROBLOSECURITY" && !c.value().is_empty()) {
            return true;
        }
    }
    if let Ok(cs) = win.cookies() {
        if cs.iter().any(|c| c.name() == ".ROBLOSECURITY" && !c.value().is_empty()) {
            return true;
        }
    }
    false
}

/// Poll a login window's title for the detect-script state code and emit
/// `login-state` on every change. When `finalize` is set (single-login
/// window), it also harvests the cookie the moment it appears — even on a
/// face-lock / captcha screen — then emits `login-complete` and closes.
/// Stops when the window closes.
fn spawn_login_state_poller(app: AppHandle, label: String, finalize: bool) {
    tauri::async_runtime::spawn(async move {
        let mut last = String::new();
        let mut done = false;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            let win = match app.get_webview_window(&label) {
                Some(w) => w,
                None => break, // window closed
            };
            let title = match win.title() {
                Ok(t) => t,
                Err(_) => break,
            };
            let code = title.split(" | ").next().unwrap_or("").trim().to_string();
            if KNOWN_STATES.contains(&code.as_str()) && code != last {
                last = code.clone();
                let _ = app.emit_to(
                    "main",
                    "login-state",
                    LoginStatePayload { label: label.clone(), state: code.clone() },
                );
            }

            // Harvest as soon as the cookie exists (even behind a lock/captcha).
            // Skip only the pure pre-auth states where no cookie can exist yet.
            if finalize && !done && code != "error" && code != "login" && code != "loading" {
                if cookie_present(&win) {
                    done = true;
                    let res = do_extract_and_add(&app, &win, None).await;
                    let payload = match res {
                        Ok(v) => LoginCompletePayload { success: true, account: Some(v), error: None },
                        Err(e) => LoginCompletePayload { success: false, account: None, error: Some(e.to_string()) },
                    };
                    let _ = app.emit_to("main", "login-complete", payload);
                    let _ = win.close();
                    break;
                }
            }
        }
    });
}

/// Open a WebView window to roblox.com/login, intercept the .ROBLOSECURITY cookie
/// after successful login, then add the account automatically.
#[tauri::command]
pub async fn open_login_window(app: AppHandle) -> AppResult<()> {
    // Close existing login window if any
    if let Some(existing) = app.get_webview_window("roblox-login") {
        let _ = existing.close();
    }

    // Fresh session: a new window + cleared browsing data so no stale cookie
    // from a previous login leaks in. (open_browser is the opposite — it keeps
    // each account's session persistent.)
    let _login_window = WebviewWindowBuilder::new(
        &app,
        "roblox-login",
        WebviewUrl::External("https://www.roblox.com/login".parse().unwrap()),
    )
    .title("Roblox Login")
    .inner_size(900.0, 700.0)
    .center()
    .resizable(true)
    .initialization_script(DETECT_JS)
    .build()
    .map_err(|e| AppError::Other(format!("Failed to create login window: {}", e)))?;

    let _ = _login_window.clear_all_browsing_data();
    let _ = _login_window.navigate("https://www.roblox.com/login".parse().unwrap());

    // Poller reports page state AND harvests the cookie the instant it appears
    // (even behind a face-lock / captcha), then adds the account and closes.
    spawn_login_state_poller(app, "roblox-login".to_string(), true);

    Ok(())
}

/// Extract the .ROBLOSECURITY cookie from a login WebView and upsert the account.
/// The cookie is the prize: if the account is face-locked/challenged so the
/// profile fetch fails, the cookie is still saved under `fallback_username`
/// (or a generated `locked-xxxxxx` name) rather than lost.
async fn do_extract_and_add(
    app: &AppHandle,
    login_window: &tauri::WebviewWindow,
    fallback_username: Option<&str>,
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

    // Try to enrich with profile info. If it fails (face-lock / challenge),
    // keep the cookie anyway under a fallback name — losing the cookie is the
    // only real failure here.
    let account = match state.roblox.get_authenticated_user(&cookie_value).await {
        Ok(user) => {
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
            account
        }
        Err(e) => {
            let name = fallback_username
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("locked-{}", &Uuid::new_v4().to_string()[..6]));
            log::warn!(
                "Profile fetch failed ({e}); saving cookie under fallback name '{name}'"
            );
            Account::new(name)
        }
    };

    let mut store = state.store.lock().map_err(|e| {
        AppError::Other(format!("Lock poisoned: {}", e))
    })?;

    // Use upsert: if same user_id or username already exists, update instead of adding duplicate
    let final_id = store.upsert_account(account, &cookie_value)?;

    // Get the final account view (may have preserved fields from existing account)
    let final_account = store.get_account(&final_id)?.clone();
    let view = final_account.to_view();

    log::info!("Account upserted via WebView login: {}", view.username);

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

    // Chrome-like user agent so Roblox serves the full desktop site
    let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

    // Build browser window — navigates to roblox.com/login first.
    // on_page_load injects cookie once we're on roblox.com domain (document.cookie
    // only works when the page origin matches the cookie domain).
    // initialization_script handles address bar + window.open interception.
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
    .initialization_script(address_bar_script)
    .on_page_load(build_cookie_handler(escaped_cookie, target_url))
    .build()
    .map_err(|e| AppError::Other(format!("Failed to create browser window: {}", e)))?;

    Ok(())
}

/// on_page_load handler: injects cookie on first roblox.com page, then redirects to target.
fn build_cookie_handler(
    escaped_cookie: String,
    target_url: String,
) -> impl Fn(tauri::WebviewWindow, tauri::webview::PageLoadPayload<'_>) + Send + Sync + 'static {
    let cookie_injected = Arc::new(AtomicBool::new(false));

    move |window, payload| {
        if payload.event() != PageLoadEvent::Finished {
            return;
        }
        let url_str = payload.url().to_string();

        // Only inject on roblox.com pages (document.cookie needs matching origin)
        if !url_str.contains("roblox.com") {
            return;
        }

        // First time on roblox.com: inject cookie and redirect to target
        if !cookie_injected.swap(true, Ordering::SeqCst) {
            let js = format!(
                r#"
                (function() {{
                    document.cookie = ".ROBLOSECURITY=expired; domain=.roblox.com; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
                    document.cookie = ".ROBLOSECURITY={cookie}; domain=.roblox.com; path=/; secure; max-age=31536000";
                    window.location.replace("{url}");
                }})();
                "#,
                cookie = escaped_cookie,
                url = target_url,
            );
            let _ = window.eval(&js);
        }
    }
}

// ===================== Bulk login =====================

/// A single credential for bulk login. Used in-memory only — never written to disk.
#[derive(Deserialize)]
pub struct Credential {
    pub username: String,
    pub password: String,
}

/// Per-account progress emitted to the main window as `bulk-progress`.
#[derive(Clone, Serialize)]
pub struct BulkProgress {
    pub index: usize,
    pub username: String,
    /// filling | captcha | locked | twostep | error | done | skipped | cancelled
    pub state: String,
    pub message: String,
}

/// A periodic thumbnail of a bulk-login window, emitted as `bulk-thumb`.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkThumb {
    pub index: usize,
    pub data_url: String,
}

// Single concurrent bulk run. Global flags keep it simple; one run at a time.
static BULK_RUNNING: AtomicBool = AtomicBool::new(false);
static BULK_CANCEL: AtomicBool = AtomicBool::new(false);
static BULK_ACTIVE_SLOTS: AtomicUsize = AtomicUsize::new(0);

/// Auto-fill step; credentials baked in as JSON string literals at __U__/__P__.
/// Lives only inside the injected script (webview memory), never disk.
const BULK_FILL_JS: &str = r#"
(function () {
  'use strict';
  if (window.top !== window.self) return;
  if (window.__RAM_FILL__) return;
  window.__RAM_FILL__ = true;
  var U = __U__, P = __P__, tries = 0;
  function setNativeValue(el, val) {
    var d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value');
    if (d && d.set) d.set.call(el, val); else el.value = val;        // React-controlled input
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }
  function fill() {
    var u = document.querySelector('#login-username');
    var p = document.querySelector('#login-password');
    var btn = document.querySelector('#login-button, button[type=submit]');
    if (u && p) {
      setNativeValue(u, U);
      setNativeValue(p, P);
      if (btn) { btn.disabled = false; btn.click(); }
      return true;
    }
    return false;
  }
  var iv = setInterval(function () { if (fill() || ++tries > 40) clearInterval(iv); }, 500);
})();
"#;

fn bulk_init_script(username: &str, password: &str) -> String {
    let u = serde_json::to_string(username).unwrap_or_else(|_| "\"\"".into());
    let p = serde_json::to_string(password).unwrap_or_else(|_| "\"\"".into());
    format!(
        "{}\n{}",
        DETECT_JS,
        BULK_FILL_JS.replace("__U__", &u).replace("__P__", &p)
    )
}

fn emit_progress(app: &AppHandle, index: usize, username: &str, state: &str, message: &str) {
    let _ = app.emit_to(
        "main",
        "bulk-progress",
        BulkProgress {
            index,
            username: username.to_string(),
            state: state.to_string(),
            message: message.to_string(),
        },
    );
}

/// How long to keep a window open for a manual captcha/lock/2sv solve before
/// giving up and moving on.
const MANUAL_SOLVE_TIMEOUT_SECS: u64 = 300;

type BulkQueue = Arc<Mutex<VecDeque<(usize, Credential)>>>;

/// Start bulk login. Spawns up to `concurrency` (1..=4) login windows that
/// auto-fill credentials. Captcha/face-lock/2sv is solved manually in the
/// window; the cookie is harvested the instant it exists. Fire-and-forget —
/// progress via `bulk-progress`, completion via `bulk-login-finished`.
#[tauri::command]
pub async fn start_bulk_login(
    credentials: Vec<Credential>,
    concurrency: usize,
    app: AppHandle,
) -> AppResult<()> {
    let total = credentials.len();
    if total == 0 {
        return Ok(());
    }
    if BULK_RUNNING.swap(true, Ordering::SeqCst) {
        return Err(AppError::Other("A bulk login is already running".into()));
    }
    BULK_CANCEL.store(false, Ordering::SeqCst);

    let n = concurrency.clamp(1, 4).min(total);
    BULK_ACTIVE_SLOTS.store(n, Ordering::SeqCst);

    let data_root = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Other(format!("Failed to get app data dir: {}", e)))?
        .join("bulk-sessions");
    let _ = std::fs::remove_dir_all(&data_root); // fresh sessions every run

    let queue: BulkQueue = Arc::new(Mutex::new(credentials.into_iter().enumerate().collect()));

    for _ in 0..n {
        let app = app.clone();
        let queue = queue.clone();
        let data_root = data_root.clone();
        tauri::async_runtime::spawn(run_bulk_slot(app, queue, data_root));
    }

    #[cfg(windows)]
    spawn_bulk_capture(app.clone(), total);

    Ok(())
}

/// Periodically screenshot each open bulk-login window and emit `bulk-thumb`.
/// Runs until the bulk run finishes.
#[cfg(windows)]
fn spawn_bulk_capture(app: AppHandle, total: usize) {
    tauri::async_runtime::spawn(async move {
        while BULK_RUNNING.load(Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            for i in 0..total {
                let label = format!("bulk-login-{}", i);
                let win = match app.get_webview_window(&label) {
                    Some(w) => w,
                    None => continue,
                };
                let raw = match win.hwnd() {
                    Ok(h) => h.0 as isize,
                    Err(_) => continue,
                };
                let thumb = tokio::task::spawn_blocking(move || {
                    crate::capture::capture_window_png_base64(raw, 240)
                })
                .await
                .ok()
                .flatten();
                if let Some(data_url) = thumb {
                    let _ = app.emit_to("main", "bulk-thumb", BulkThumb { index: i, data_url });
                }
            }
        }
    });
}

/// Cancel an in-progress bulk login. Slots observe the flag, close their
/// windows, and exit.
#[tauri::command]
pub fn cancel_bulk_login() {
    BULK_CANCEL.store(true, Ordering::SeqCst);
}

/// One worker slot: pulls accounts off the queue and logs them in one at a
/// time, so at most `concurrency` windows are ever open.
async fn run_bulk_slot(app: AppHandle, queue: BulkQueue, data_root: PathBuf) {
    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

    loop {
        if BULK_CANCEL.load(Ordering::SeqCst) {
            break;
        }
        // Drop the lock before any await (std Mutex guard isn't held across await).
        let next = { queue.lock().ok().and_then(|mut q| q.pop_front()) };
        let (index, cred) = match next {
            Some(x) => x,
            None => break,
        };

        let label = format!("bulk-login-{}", index);
        emit_progress(&app, index, &cred.username, "filling", "Opening login window…");

        let dir = data_root.join(index.to_string());
        if let Err(e) = std::fs::create_dir_all(&dir) {
            emit_progress(&app, index, &cred.username, "error", &format!("data dir: {}", e));
            continue;
        }

        let build = WebviewWindowBuilder::new(
            &app,
            &label,
            WebviewUrl::External("https://www.roblox.com/login".parse().unwrap()),
        )
        .title(format!("{} — bulk login", cred.username))
        .inner_size(900.0, 700.0)
        .resizable(true)
        .user_agent(ua)
        .data_directory(dir)
        .initialization_script(&bulk_init_script(&cred.username, &cred.password))
        .build();

        let win = match build {
            Ok(w) => w,
            Err(e) => {
                emit_progress(&app, index, &cred.username, "error", &format!("window: {}", e));
                continue;
            }
        };
        let _ = win.clear_all_browsing_data(); // fresh session
        let _ = win.navigate("https://www.roblox.com/login".parse().unwrap());

        // Poll this window until terminal.
        let started = Instant::now();
        let mut last = String::new();
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;

            if BULK_CANCEL.load(Ordering::SeqCst) {
                let _ = win.close();
                emit_progress(&app, index, &cred.username, "cancelled", "Cancelled");
                break;
            }

            let w = match app.get_webview_window(&label) {
                Some(w) => w,
                None => {
                    emit_progress(&app, index, &cred.username, "skipped", "Window closed");
                    break;
                }
            };
            let title = match w.title() {
                Ok(t) => t,
                Err(_) => {
                    emit_progress(&app, index, &cred.username, "skipped", "Window closed");
                    break;
                }
            };
            let code = title.split(" | ").next().unwrap_or("").trim().to_string();

            // Harvest as soon as the cookie exists — even behind a lock/captcha.
            if code != "error" && code != "login" && code != "loading" && cookie_present(&w) {
                match do_extract_and_add(&app, &w, Some(&cred.username)).await {
                    Ok(v) => emit_progress(&app, index, &v.username, "done", "Added"),
                    Err(e) => emit_progress(&app, index, &cred.username, "error", &e.to_string()),
                }
                let _ = w.close();
                break;
            }

            if KNOWN_STATES.contains(&code.as_str()) && code != last {
                last = code.clone();
                let (st, msg): (&str, &str) = match code.as_str() {
                    "captcha" => ("captcha", "Captcha — solve it in the window"),
                    "locked" => ("locked", "Face-lock — confirm in the window"),
                    "twostep" => ("twostep", "2-step — enter code in the window"),
                    "error" => ("error", "Incorrect username or password"),
                    _ => ("filling", "Signing in…"),
                };
                emit_progress(&app, index, &cred.username, st, msg);
            }

            // Wrong password is terminal; let the user glimpse it, then move on.
            if code == "error" {
                tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
                let _ = w.close();
                break;
            }

            // Give up on a stuck manual challenge after the timeout.
            if matches!(code.as_str(), "captcha" | "locked" | "twostep")
                && started.elapsed().as_secs() > MANUAL_SOLVE_TIMEOUT_SECS
            {
                emit_progress(&app, index, &cred.username, "skipped", "Timed out waiting for manual solve");
                let _ = w.close();
                break;
            }
        }

        // Let WebView2 release the data-dir lock before the next account.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    // Last slot out turns off the lights.
    if BULK_ACTIVE_SLOTS.fetch_sub(1, Ordering::SeqCst) == 1 {
        BULK_RUNNING.store(false, Ordering::SeqCst);
        let _ = app.emit_to("main", "bulk-login-finished", ());
    }
}

