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

/// WebView2 freezes JS timers in backgrounded/occluded windows, which stalls
/// the human-typing loop mid-word once a second login window covers the first.
/// These Chromium flags keep timers running when the window isn't focused.
/// (additional_browser_args REPLACES wry's default, so the defaults are kept here.)
/// `--lang=en-US` forces Roblox to render the login/challenge pages in English
/// so the state-detection text below matches — a localized (e.g. Indonesian)
/// page would otherwise slip past "locked"/"press and hold" and stall.
const NO_THROTTLE_ARGS: &str = "--lang=en-US --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --disable-background-timer-throttling --disable-backgrounding-occluded-windows --disable-renderer-backgrounding";

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
    // Only an explicit "account locked" screen is a face-lock. Generic
    // human-check text ("verify you're a human", press-and-hold) is the routine
    // login captcha — flagging it as locked marked normal accounts Face-lock.
    if (/account\s+(?:has been\s+|is\s+)?locked|akun.{0,20}terkunci|mem?buka\s*kunci\s*akun/i.test(txt)) return ['locked', 'Account locked — confirm you are human'];
    if (/suspicious activity|confirm(?:ing)?\s+that\s+you['’]?re\s+a?\s*human|verify\s+(?:that\s+)?you['’]?re\s+a?\s*human|aktivitas.{0,10}mencurigakan|(?:k|meng)onfirmasi.{0,30}manusia|verifikasi.{0,20}manusia/i.test(txt)) return ['captcha', 'Solve the challenge to continue'];
    if (/two[- ]step|2-step|verification code|security code|authenticator/i.test(txt) || /2sv|two-step/i.test(href)) return ['twostep', 'Enter your verification code'];
    if (/incorrect username or password|wasn['’]?t quite right|didn['’]?t match|invalid password/i.test(txt)) return ['error', 'Incorrect username or password'];
    if (document.querySelector('#login-username, input#login-username, form#login-form')) return ['login', 'Waiting for login'];
    return ['loading', 'Loading…'];
  }

  // Report state via the URL hash (document.title does NOT propagate to the OS
  // window title in a Tauri webview, so window.title() can't be read back).
  // Rust reads it with win.url().fragment(). Roblox uses path routing, so a
  // rams= fragment is ignored by its router and never reloads.
  var lastCode = '';
  function tick() {
    try {
      var code = classify()[0];
      if (code !== lastCode) { lastCode = code; location.hash = 'rams=' + code; }
    } catch (e) {}
  }

  tick();
  setInterval(tick, 700);
  if (document.body) new MutationObserver(tick).observe(document.body, { childList: true, subtree: true });
  else document.addEventListener('DOMContentLoaded', function () {
    tick();
    new MutationObserver(tick).observe(document.body, { childList: true, subtree: true });
  });

  // Auto press-and-hold: Roblox's "confirm you're human" gate uses a hold
  // button in the TOP frame (we already skip the arkose iframe above, so a
  // FunCaptcha hold is never touched). The hold duration is set by Roblox, so
  // press and keep holding until the button goes away — don't release early.
  // The actual face/selfie scan still needs the real person.
  var held = null, mv = null, cap = null;
  function findHold() {
    var els = document.querySelectorAll('button,[role="button"],[class*="hold" i]');
    for (var i = 0; i < els.length; i++) {
      var el = els[i];
      if (!el.offsetParent) continue; // visible only
      var t = (el.innerText || el.textContent || '').trim();
      if (/press\s*(?:&|and)?\s*hold|hold\s*(?:to|down|and)|tekan\s*(?:&|dan)?\s*tahan|tahan\s*(?:untuk|dan)/i.test(t)) return el;
    }
    return null;
  }
  function pe(el, type) {
    var r = el.getBoundingClientRect();
    var o = { bubbles: true, cancelable: true, composed: true, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: 0, buttons: type === 'pointerup' ? 0 : 1, pointerId: 1, pointerType: 'mouse', isPrimary: true };
    try { el.dispatchEvent(new PointerEvent(type, o)); } catch (e) {}
    var m = type === 'pointerdown' ? 'mousedown' : type === 'pointerup' ? 'mouseup' : 'mousemove';
    try { el.dispatchEvent(new MouseEvent(m, o)); } catch (e) {}
  }
  function releaseHold() {
    if (!held) return;
    pe(held, 'pointerup');
    clearInterval(mv); clearTimeout(cap); held = null;
  }
  setInterval(function () {
    var el = findHold();
    if (el && !held) {
      held = el;
      try { el.focus && el.focus(); } catch (e) {}
      pe(el, 'pointerdown');
      mv = setInterval(function () { if (held) pe(held, 'pointermove'); }, 180);
      cap = setTimeout(releaseHold, 15000); // safety: never hold forever
    } else if (!el && held) {
      releaseHold(); // gate passed → button gone
    }
  }, 500);

  // Auto-advance the "Account locked / Akun terkunci" intro dialog past its
  // Continue button so the flow reaches the human-check step on its own. Only
  // the primary continue action — NEVER "Log out / Keluar". The real
  // verification (QR scan / press-and-hold / selfie) still needs the person.
  var advanced = new WeakSet();
  setInterval(function () {
    var els = document.querySelectorAll('button,[role="button"]');
    for (var i = 0; i < els.length; i++) {
      var el = els[i];
      if (!el.offsetParent || advanced.has(el)) continue; // visible, once each
      var t = (el.innerText || el.textContent || '').trim();
      if (/log\s*out|keluar|batal|cancel/i.test(t)) continue;      // never these
      if (/^\s*(?:lanjutkan|continue|unlock|buka kunci)\s*$/i.test(t)) {
        advanced.add(el);
        try { el.click(); } catch (e) {}
        break;
      }
    }
  }, 600);
})();
"#;

/// Is a non-empty `.ROBLOSECURITY` cookie present in this window right now?
/// Cheap/local (no network). The cookie is the prize: once it exists we can
/// harvest it even behind a face-lock / captcha interstitial.
fn cookie_present(win: &tauri::WebviewWindow) -> bool {
    read_roblosecurity(win).is_some()
}

/// Current non-empty `.ROBLOSECURITY` value in this window, if any.
/// Blocks on WebView2 — call from an async task, never a sync page-load handler.
fn read_roblosecurity(win: &tauri::WebviewWindow) -> Option<String> {
    let url: tauri::Url = "https://www.roblox.com".parse().unwrap();
    let find = |cs: Vec<tauri::webview::Cookie<'static>>| {
        cs.into_iter()
            .find(|c| c.name() == ".ROBLOSECURITY" && !c.value().is_empty())
            .map(|c| c.value().to_string())
    };
    win.cookies_for_url(url).ok().and_then(find)
        .or_else(|| win.cookies().ok().and_then(find))
}

/// Chrome-like UA used by Open Browser; also exported in "Copy header".
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Show a short message inside an Open Browser window, optionally copying
/// `copy` to the clipboard (the click that triggered it is the user gesture).
fn browser_toast(win: &tauri::WebviewWindow, msg: &str, copy: Option<&str>) {
    let js = format!(
        r#"(function(){{
            var msg = {msg}, text = {copy};
            function toast(m){{
                var t = document.getElementById('ram-toast');
                if (!t) {{ t = document.createElement('div'); t.id = 'ram-toast'; document.body.appendChild(t); }}
                t.textContent = m;
                t.style.cssText = 'position:fixed;top:50px;right:12px;z-index:2147483647;background:#1a1a2e;color:#f0f0f0;border:1px solid #5865f2;border-radius:8px;padding:10px 14px;font:13px system-ui,sans-serif;max-width:420px;box-shadow:0 6px 20px rgba(0,0,0,.4)';
                clearTimeout(window.__ramToastT); window.__ramToastT = setTimeout(function(){{ t.remove(); }}, 4000);
            }}
            if (text === null) {{ toast(msg); return; }}
            function fallback(){{
                var ta = document.createElement('textarea'); ta.value = text;
                ta.style.cssText = 'position:fixed;top:-1000px'; document.body.appendChild(ta);
                ta.select(); var ok = false; try {{ ok = document.execCommand('copy'); }} catch (e) {{}}
                ta.remove(); toast(ok ? msg : 'Copy blocked by the page, try again.');
            }}
            if (navigator.clipboard) navigator.clipboard.writeText(text).then(function(){{ toast(msg); }}, fallback);
            else fallback();
        }})();"#,
        msg = serde_json::to_string(msg).unwrap_or_default(),
        copy = copy.map(|c| serde_json::to_string(c).unwrap_or_default()).unwrap_or_else(|| "null".into()),
    );
    let _ = win.eval(&js);
}

/// "Save cookie" button: store this window's current cookie on the account,
/// but only if it belongs to the SAME Roblox user (never overwrite with
/// another account's session).
async fn save_browser_cookie(app: AppHandle, win: tauri::WebviewWindow, account_id: Uuid) {
    let Some(fresh) = read_roblosecurity(&win) else {
        browser_toast(&win, "No Roblox login in this window. Log in first.", None);
        return;
    };
    let state = app.state::<AppState>();
    let (stored, user_id, username) = {
        let Ok(store) = state.store.lock() else { return };
        let Ok(acc) = store.get_account(&account_id) else { return };
        let stored = store.get_cookie(&account_id).unwrap_or_default();
        (stored, acc.user_id, acc.username.clone())
    };
    if stored.trim_start_matches(".ROBLOSECURITY=") == fresh {
        browser_toast(&win, "Cookie unchanged, already saved.", None);
        return;
    }
    let user = match state.roblox.get_authenticated_user(&fresh).await {
        Ok(u) => u,
        Err(e) => {
            browser_toast(&win, &format!("Not saved: this login isn't usable yet ({e})."), None);
            return;
        }
    };
    if user_id.is_some_and(|id| id != user.id) {
        browser_toast(&win, &format!("Not saved: logged in as {}, not {}.", user.name, username), None);
        return;
    }
    let saved = state.store.lock().ok().is_some_and(|mut store| {
        let ok = store.update_cookie(&account_id, &fresh).is_ok();
        if ok {
            let _ = store.set_flag(&account_id, "cookie_dead", false);
        }
        ok
    });
    if saved {
        log::info!("[{username}] saved new cookie from browser window");
        browser_toast(&win, &format!("New cookie saved for {username}."), None);
        let _ = app.emit("browser-cookie", serde_json::json!({
            "saved": true,
            "message": format!("New cookie saved for {}.", username),
        }));
    } else {
        browser_toast(&win, "Saving the cookie failed.", None);
    }
}

/// "Copy header" button: Cookie + User-Agent of this browser session.
fn copy_browser_header(win: tauri::WebviewWindow) {
    match read_roblosecurity(&win) {
        Some(c) => browser_toast(
            &win,
            "Header copied.",
            Some(&format!("Cookie: .ROBLOSECURITY={c}\nUser-Agent: {BROWSER_UA}")),
        ),
        None => browser_toast(&win, "No Roblox login in this window, nothing to copy.", None),
    }
}

/// Extract the detect-script state code from a window URL's `#rams=<code>`
/// fragment. Empty string when absent.
fn url_state(u: &tauri::Url) -> String {
    u.fragment()
        .and_then(|f| f.strip_prefix("rams="))
        .unwrap_or("")
        .to_string()
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
        let mut saw_lock = false; // a face-lock screen appeared before harvest
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            let win = match app.get_webview_window(&label) {
                Some(w) => w,
                None => break, // window closed
            };
            let code = match win.url() {
                Ok(u) => url_state(&u),
                Err(_) => break,
            };
            if KNOWN_STATES.contains(&code.as_str()) && code != last {
                last = code.clone();
                if code == "locked" { saw_lock = true; }
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
                    let res = do_extract_and_add(&app, &win, None, saw_lock).await;
                    let payload = match res {
                        Ok((v, _cookie)) => LoginCompletePayload { success: true, account: Some(v), error: None },
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
    .additional_browser_args(NO_THROTTLE_ARGS)
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
/// Returns the stored account view plus the raw cookie it harvested — the
/// caller decides whether to surface the cookie (bulk export) or drop it.
async fn do_extract_and_add(
    app: &AppHandle,
    login_window: &tauri::WebviewWindow,
    fallback_username: Option<&str>,
    facelocked: bool,
) -> AppResult<(AccountView, String)> {
    // Try cookies_for_url first, then fall back to all cookies
    let roblox_url: tauri::Url = "https://www.roblox.com".parse().unwrap();
    let mut cookies = login_window
        .cookies_for_url(roblox_url)
        .map_err(|e| AppError::Other(format!("Failed to get cookies for URL: {}", e)))?;

    // Never log cookie names/values — .ROBLOSECURITY is a full session secret.
    log::info!("cookies_for_url returned {} cookies", cookies.len());

    // If cookies_for_url didn't find it, try all cookies
    if !cookies.iter().any(|c| c.name() == ".ROBLOSECURITY") {
        cookies = login_window
            .cookies()
            .map_err(|e| AppError::Other(format!("Failed to get all cookies: {}", e)))?;
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

    // Persist the face-lock flag AFTER upsert — upsert keeps the existing
    // account's `fields`, so this has to be written on the stored record.
    store.set_facelock(&final_id, facelocked)?;

    // Get the final account view (may have preserved fields from existing account)
    let final_account = store.get_account(&final_id)?.clone();
    let view = final_account.to_view();

    log::info!("Account upserted via WebView login: {}", view.username);

    Ok((view, cookie_value))
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

        // === Unlock challenge watcher ===
        // When Roblox answers with a challenge (rblx-challenge-type header) on the
        // moderation "unlock" flow, report it to RAM via a short-lived page title.
        function ramReport(url, status, type, body) {
            var path = location.pathname;
            if (!/usermoderation/i.test(url) && !/not-approved/i.test(path)) return;
            var old = document.title;
            document.title = ['RAM_CHALLENGE', type || 'none', status, String(url).split('?')[0], path, String(body || '').slice(0, 2000)].join('|');
            setTimeout(function() { if (document.title.indexOf('RAM_CHALLENGE|') === 0) document.title = old; }, 500);
        }
        var _fetch = window.fetch;
        if (_fetch) window.fetch = function(input, init) {
            var url = typeof input === 'string' ? input : (input && input.url) || '';
            return _fetch.apply(this, arguments).then(function(res) {
                try {
                    var type = res.headers.get('rblx-challenge-type');
                    if (type || /usermoderation/i.test(url)) {
                        res.clone().text().then(function(b) { ramReport(url, res.status, type, b); }, function() {});
                    }
                } catch (e) {}
                return res;
            });
        };
        var _xopen = XMLHttpRequest.prototype.open;
        XMLHttpRequest.prototype.open = function(method, url) {
            var xhr = this;
            xhr.addEventListener('loadend', function() {
                try {
                    var type = xhr.getResponseHeader('rblx-challenge-type');
                    if (type || /usermoderation/i.test(url)) {
                        var b = (xhr.responseType === '' || xhr.responseType === 'text') ? xhr.responseText : '';
                        ramReport(url, xhr.status, type, b);
                    }
                } catch (e) {}
            });
            return _xopen.apply(this, arguments);
        };

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
            [['Save cookie', 'save-cookie', 'Store this login as the account cookie'],
             ['Copy header', 'copy-header', 'Copy Cookie + User-Agent headers']].forEach(function(b) {
                var btn = document.createElement('button');
                btn.className = 'ram-btn ram-btn-text';
                btn.textContent = b[0];
                btn.title = b[2];
                btn.onclick = function() { location.href = 'https://ram.invalid/' + b[1]; };
                bar.appendChild(btn);
            });

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
                '#ram-nav-bar .ram-btn-text { font-size: 12px; white-space: nowrap; }',
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
    let user_agent = BROWSER_UA;

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
    // Bar buttons "navigate" to https://ram.invalid/<action>; catch & cancel it.
    .on_navigation({
        let app = app.clone();
        let label = window_label.clone();
        move |url| {
            if url.host_str() != Some("ram.invalid") {
                return true;
            }
            if let Some(win) = app.get_webview_window(&label) {
                let action = url.path().to_string();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    match action.as_str() {
                        "/save-cookie" => save_browser_cookie(app, win, uuid).await,
                        "/copy-header" => copy_browser_header(win),
                        _ => {}
                    }
                });
            }
            false
        }
    })
    .on_document_title_changed({
        let app = app.clone();
        move |win, title| {
            if let Some(rest) = title.strip_prefix("RAM_CHALLENGE|") {
                record_unlock_challenge(&app, &win, uuid, rest);
            }
        }
    })
    .build()
    .map_err(|e| AppError::Other(format!("Failed to create browser window: {}", e)))?;

    Ok(())
}

/// Unlock flow on roblox.com/not-approved: remember which challenge Roblox
/// asked for (captcha, face/biometric, ...) so it shows in the Status column.
/// `report` = "type|status|url|page|body" from the page hook.
fn record_unlock_challenge(app: &AppHandle, win: &tauri::WebviewWindow, account_id: Uuid, report: &str) {
    let mut p = report.splitn(5, '|');
    let (kind, status, url, page, body) = (
        p.next().unwrap_or("").to_lowercase(),
        p.next().unwrap_or("").parse::<u16>().unwrap_or(0),
        p.next().unwrap_or(""),
        p.next().unwrap_or(""),
        p.next().unwrap_or(""),
    );
    log::info!("unlock watch: type={kind} status={status} url={url} page={page} body={body}");
    let challenge = (kind != "none").then_some(kind.as_str());
    let Some(message) = apply_unlock_result(app, account_id, challenge, status, url.contains("reactivate")) else { return };
    browser_toast(win, &message, None);
    let _ = app.emit("browser-cookie", serde_json::json!({ "saved": true, "message": message }));
}

/// Store what an Unlock press returned. Some(message) when something changed.
pub fn apply_unlock_result(app: &AppHandle, account_id: Uuid, challenge: Option<&str>, status: u16, is_reactivate: bool) -> Option<String> {
    let state = app.state::<AppState>();
    let mut store = state.store.lock().ok()?;
    let username = store.get_account(&account_id).map(|a| a.username.clone()).unwrap_or_default();
    if let Some(kind) = challenge {
        let kind = kind.to_lowercase();
        let _ = store.set_flag(&account_id, "moderated", true);
        let _ = store.set_field(&account_id, "unlock_challenge", Some(&kind));
        let _ = store.set_field(&account_id, "unlock_challenge_at", Some(&chrono::Utc::now().to_rfc3339()));
        // Biometric (face) check to unlock = face-locked account.
        let face = ["face", "bio", "selfie", "liveness", "ident", "age"].iter().any(|k| kind.contains(k));
        if face {
            let _ = store.set_flag(&account_id, "facelock", true);
        }
        let label = if face { "Facelock" } else if kind.contains("captcha") { "Captcha Login" } else { kind.as_str() };
        Some(format!("{username}: unlock needs {label}."))
    } else if is_reactivate && (200..300).contains(&status) {
        let _ = store.set_flag(&account_id, "moderated", false);
        Some(format!("{username}: account unlocked."))
    } else {
        None
    }
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
#[derive(Deserialize, Clone)]
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

/// Harvested cookie for a bulk row, emitted as `bulk-cookie` ONLY when the
/// user turned on "export cookies" — the frontend pairs it with the password
/// it already holds to build a `user:pass:cookie` line.
#[derive(Clone, Serialize)]
pub struct BulkCookie {
    pub index: usize,
    pub cookie: String,
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
  function rand(a, b) { return a + Math.floor(Math.random() * (b - a)); }

  // Type char-by-char, re-querying the element each step and resuming from the
  // common prefix so a background reset/remount can't truncate it.
  function typeHuman(selector, text, done) {
    var misses = 0;
    (function step() {
      var el = document.querySelector(selector);
      if (!el) { if (++misses > 120) { done(); return; } setTimeout(step, 120); return; }
      var i = 0;
      while (i < text.length && el.value.charAt(i) === text.charAt(i)) i++;
      if (i >= text.length) { el.dispatchEvent(new Event('change', { bubbles: true })); done(); return; }
      if (i === 0) el.focus();
      var ch = text.charAt(i);
      var setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value');
      el.dispatchEvent(new KeyboardEvent('keydown', { key: ch, bubbles: true }));
      if (setter && setter.set) setter.set.call(el, text.slice(0, i + 1)); else el.value = text.slice(0, i + 1);
      el.dispatchEvent(new InputEvent('input', { data: ch, inputType: 'insertText', bubbles: true }));
      el.dispatchEvent(new KeyboardEvent('keyup', { key: ch, bubbles: true }));
      setTimeout(step, rand(70, 150));
    })();
  }
  function pressEnter(el) {
    if (!el) return;
    el.focus();
    ['keydown', 'keypress', 'keyup'].forEach(function (t) {
      el.dispatchEvent(new KeyboardEvent(t, { key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true, cancelable: true }));
    });
  }
  // Submit via BOTH the button click and Enter in the password field — on
  // Roblox the button click can get swallowed by the passkey (WebAuthn) flow,
  // while Enter goes straight through the password-login path.
  function clickLogin() {
    var b = document.querySelector('#login-button, button[type=submit]');
    if (b) {
      b.disabled = false;
      var o = { bubbles: true, cancelable: true, view: window, button: 0 };
      try { b.dispatchEvent(new MouseEvent('mousedown', o)); } catch (e) {}
      try { b.dispatchEvent(new MouseEvent('mouseup', o)); } catch (e) {}
      b.click();
    }
    pressEnter(document.querySelector('#login-password'));
    return !!b;
  }
  function busy() {
    return /captcha|challenge|verify|incorrect|two-step|verification|suspicious|locked|one-time|terkunci|verifikasi|mencurigakan|tekan.{0,3}tahan|kode/i.test(document.body.innerText)
      || !!document.querySelector('iframe[src*=arkose],iframe[src*=funcaptcha]');
  }

  var wait = setInterval(function () {
    var u = document.querySelector('#login-username');
    var p = document.querySelector('#login-password');
    if (!u || !p) { if (++tries > 80) clearInterval(wait); return; } // wait for React to mount (~24s cap)
    clearInterval(wait);
    typeHuman('#login-username', U, function () {
      setTimeout(function () {
        typeHuman('#login-password', P, function () {
          setTimeout(function () {
            clickLogin();
            // Conservative retries only while the plain login form is still idle
            // (no challenge/error yet). Spaced out — never hammered.
            var retries = 0;
            var rt = setInterval(function () {
              if (!document.querySelector('#login-username') || busy() || ++retries > 4) { clearInterval(rt); return; }
              clickLogin();
            }, 2500);
          }, rand(700, 1100));
        });
      }, rand(400, 700));
    });
  }, 300);
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

type BulkQueue = Arc<Mutex<VecDeque<(usize, Credential, usize)>>>;

/// How long a window may sit on a blank / un-loaded login page (proxy never
/// connected) before we give up on that proxy and rotate to the next one.
/// Only armed when the account is actually behind a proxy.
/// ponytail: time heuristic, not a load-failure event — a genuinely-slow
/// proxy taking >60s to show the login page gets rotated too.
const STUCK_PROXY_TIMEOUT_SECS: u64 = 60;

/// A proxy line parsed into its address and optional Basic-auth blob.
struct ProxySpec {
    host_port: String,
    auth_b64: Option<String>,
}

/// Parse one proxy line. Accepts `host:port`, `user:pass@host:port`,
/// `host:port:user:pass`, with an optional `http(s)://` scheme. SOCKS is
/// unsupported (returns None). None also for blank/garbage lines.
fn parse_proxy(line: &str) -> Option<ProxySpec> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let line = match line.split_once("://") {
        Some((scheme, rest)) => {
            if scheme.eq_ignore_ascii_case("socks5") || scheme.eq_ignore_ascii_case("socks4") {
                return None; // ponytail: SOCKS skipped — different protocol, add if asked
            }
            rest
        }
        None => line,
    };
    let (host_port, creds) = if let Some((creds, hp)) = line.split_once('@') {
        (hp.to_string(), Some(creds.to_string()))
    } else {
        let parts: Vec<&str> = line.split(':').collect();
        match parts.len() {
            2 => (line.to_string(), None),
            4 => (
                format!("{}:{}", parts[0], parts[1]),
                Some(format!("{}:{}", parts[2], parts[3])),
            ),
            _ => return None,
        }
    };
    if host_port.split(':').count() != 2 {
        return None;
    }
    let auth_b64 = creds.map(|c| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(c.as_bytes())
    });
    Some(ProxySpec { host_port, auth_b64 })
}

/// Resolve each proxy line to a `--proxy-server` value. A no-auth proxy maps
/// straight to `http://host:port`. An authed proxy — WebView2's flag can't
/// carry credentials — maps to a local relay `http://127.0.0.1:<port>` that
/// forwards to it with the `Proxy-Authorization` header injected. Unparseable
/// lines become `None` (that account connects directly).
async fn resolve_proxies(lines: Vec<String>) -> Vec<Option<String>> {
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        match parse_proxy(&line) {
            None => out.push(None),
            Some(ProxySpec { host_port, auth_b64: None }) => {
                out.push(Some(format!("http://{}", host_port)));
            }
            Some(ProxySpec { host_port, auth_b64: Some(auth) }) => {
                match tokio::net::TcpListener::bind("127.0.0.1:0").await {
                    Ok(listener) => {
                        let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
                        tauri::async_runtime::spawn(run_auth_proxy_relay(listener, host_port, auth));
                        out.push(Some(format!("http://127.0.0.1:{}", port)));
                    }
                    Err(_) => out.push(None),
                }
            }
        }
    }
    out
}

/// Local HTTP proxy that chains to an authenticated upstream proxy, adding the
/// `Proxy-Authorization` header WebView2 can't supply on its own. Speaks both
/// CONNECT (HTTPS, all that Roblox login needs) and plain HTTP by reading the
/// request head, inserting the auth line, then byte-splicing the rest through.
/// Accepts until the bulk run ends.
async fn run_auth_proxy_relay(listener: tokio::net::TcpListener, upstream: String, auth_b64: String) {
    loop {
        let accepted = tokio::select! {
            a = listener.accept() => a,
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                if BULK_RUNNING.load(Ordering::SeqCst) { continue; }
                break;
            }
        };
        let client = match accepted {
            Ok((c, _)) => c,
            Err(_) => continue,
        };
        let upstream = upstream.clone();
        let auth = auth_b64.clone();
        tauri::async_runtime::spawn(async move {
            let _ = relay_conn(client, upstream, auth).await;
        });
    }
}

async fn relay_conn(mut client: tokio::net::TcpStream, upstream: String, auth_b64: String) -> std::io::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    // Read the request head (up to the blank line that ends the headers).
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut tmp = [0u8; 1024];
    while find_subslice(&buf, b"\r\n\r\n").is_none() {
        let n = client.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > 32 * 1024 {
            break; // runaway guard
        }
    }
    let hdr_end = find_subslice(&buf, b"\r\n\r\n").map(|i| i + 4).unwrap_or(buf.len());
    let line_end = find_subslice(&buf, b"\r\n").map(|i| i + 2).unwrap_or(0);

    // Request line, then our injected auth header, then the rest of the head.
    let mut head: Vec<u8> = Vec::with_capacity(buf.len() + 64);
    head.extend_from_slice(&buf[..line_end]);
    head.extend_from_slice(format!("Proxy-Authorization: Basic {}\r\n", auth_b64).as_bytes());
    head.extend_from_slice(&buf[line_end..hdr_end]);

    let mut server = tokio::net::TcpStream::connect(&upstream).await?;
    server.write_all(&head).await?;
    if hdr_end < buf.len() {
        server.write_all(&buf[hdr_end..]).await?; // any bytes already read past the head
    }
    tokio::io::copy_bidirectional(&mut client, &mut server).await?;
    Ok(())
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Start bulk login. Spawns up to `concurrency` (1..=4) login windows that
/// auto-fill credentials. Captcha/face-lock/2sv is solved manually in the
/// window; the cookie is harvested the instant it exists. Fire-and-forget —
/// progress via `bulk-progress`, completion via `bulk-login-finished`.
#[tauri::command]
pub async fn start_bulk_login(
    credentials: Vec<Credential>,
    concurrency: usize,
    export_cookies: bool,
    gap_ms: u64,
    proxies: Vec<String>,
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

    // Map account i → proxies[i % len]; rotation retries walk further along the
    // list. Authed proxies get a local auth-injecting relay (started here).
    let proxies = Arc::new(resolve_proxies(proxies).await);

    let queue: BulkQueue =
        Arc::new(Mutex::new(credentials.into_iter().enumerate().map(|(i, c)| (i, c, 0usize)).collect()));

    for _ in 0..n {
        let app = app.clone();
        let queue = queue.clone();
        let data_root = data_root.clone();
        let proxies = proxies.clone();
        tauri::async_runtime::spawn(run_bulk_slot(app, queue, data_root, export_cookies, gap_ms, proxies));
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

/// Bring a bulk-login window to the front so the user can solve a captcha /
/// Arkose hold / 2-step manually. Invoked when a status card is clicked.
#[tauri::command]
pub fn focus_bulk_window(index: usize, app: AppHandle) -> AppResult<()> {
    let label = format!("bulk-login-{}", index);
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        Ok(())
    } else {
        Err(AppError::Other("That login window is no longer open".into()))
    }
}

/// One worker slot: pulls accounts off the queue and logs them in one at a
/// time, so at most `concurrency` windows are ever open.
async fn run_bulk_slot(app: AppHandle, queue: BulkQueue, data_root: PathBuf, export_cookies: bool, gap_ms: u64, proxies: Arc<Vec<Option<String>>>) {
    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

    loop {
        if BULK_CANCEL.load(Ordering::SeqCst) {
            break;
        }
        // Drop the lock before any await (std Mutex guard isn't held across await).
        let next = { queue.lock().ok().and_then(|mut q| q.pop_front()) };
        let (index, cred, attempt) = match next {
            Some(x) => x,
            None => break,
        };

        // This attempt's proxy (if any): assigned one, rotated by attempt.
        let proxy_arg = if proxies.is_empty() {
            None
        } else {
            proxies[(index + attempt) % proxies.len()].clone()
        };
        let browser_args = match &proxy_arg {
            Some(p) => format!("{} --proxy-server={}", NO_THROTTLE_ARGS, p),
            None => NO_THROTTLE_ARGS.to_string(),
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
        .additional_browser_args(&browser_args)
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
        let mut challenged = String::new(); // captcha/locked/twostep seen before harvest
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
            let code = match w.url() {
                Ok(u) => url_state(&u),
                Err(_) => {
                    emit_progress(&app, index, &cred.username, "skipped", "Window closed");
                    break;
                }
            };

            // Report state transitions FIRST, so a captcha / face-lock / 2sv is
            // surfaced even when the cookie gets harvested on this same tick.
            if KNOWN_STATES.contains(&code.as_str()) && code != last {
                last = code.clone();
                // A login captcha is routine (everyone gets one) — not an
                // account condition, so it's not recorded as a challenge flag.
                if matches!(code.as_str(), "locked" | "twostep") {
                    challenged = code.clone();
                }
                let (st, msg): (&str, &str) = match code.as_str() {
                    "captcha" => ("captcha", "Captcha — solve it in the window"),
                    "locked" => ("locked", "Face-lock — confirm in the window"),
                    "twostep" => ("twostep", "2-step — enter code in the window"),
                    "error" => ("error", "Incorrect username or password"),
                    _ => ("filling", "Signing in…"),
                };
                emit_progress(&app, index, &cred.username, st, msg);
            }

            // Harvest as soon as the cookie exists — even behind a lock/captcha.
            // Note which challenge was on screen so the user knows the account
            // was face-locked etc. even though the cookie was saved.
            if code != "error" && code != "login" && code != "loading" && cookie_present(&w) {
                let note = match challenged.as_str() {
                    "locked" => "Cookie saved — account was face-locked",
                    "twostep" => "Cookie saved — 2-step was shown",
                    _ => "Added",
                };
                match do_extract_and_add(&app, &w, Some(&cred.username), challenged == "locked").await {
                    Ok((v, cookie)) => {
                        emit_progress(&app, index, &v.username, "done", note);
                        if export_cookies {
                            let _ = app.emit_to("main", "bulk-cookie", BulkCookie { index, cookie });
                        }
                    }
                    Err(e) => emit_progress(&app, index, &cred.username, "error", &e.to_string()),
                }
                let _ = w.close();
                break;
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

            // Dead/stuck proxy: never got past the blank/login page. Rotate to
            // the next proxy and retry, up to the number of proxies we have.
            if proxy_arg.is_some()
                && matches!(code.as_str(), "" | "login" | "loading")
                && started.elapsed().as_secs() > STUCK_PROXY_TIMEOUT_SECS
            {
                let _ = w.close();
                if attempt + 1 < proxies.len() {
                    emit_progress(&app, index, &cred.username, "queued", "Proxy stuck — retrying with next proxy…");
                    if let Ok(mut q) = queue.lock() {
                        q.push_back((index, cred.clone(), attempt + 1));
                    }
                } else {
                    emit_progress(&app, index, &cred.username, "error", "All proxies failed or timed out");
                }
                break;
            }
        }

        // Let WebView2 release the data-dir lock before the next account.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        // User-set gap between accounts: the first `concurrency` windows open at
        // once, then each slot waits this long before picking up the next one —
        // spaces out the load so Roblox isn't hit by a burst.
        if gap_ms > 0 && !BULK_CANCEL.load(Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(gap_ms)).await;
        }
    }

    // Last slot out turns off the lights.
    if BULK_ACTIVE_SLOTS.fetch_sub(1, Ordering::SeqCst) == 1 {
        BULK_RUNNING.store(false, Ordering::SeqCst);
        let _ = app.emit_to("main", "bulk-login-finished", ());
    }
}

#[cfg(test)]
mod proxy_tests {
    use super::parse_proxy;

    // base64("u:p") = "dTpw"
    #[test]
    fn parses_every_accepted_shape() {
        let plain = parse_proxy("1.2.3.4:8080").unwrap();
        assert_eq!(plain.host_port, "1.2.3.4:8080");
        assert!(plain.auth_b64.is_none());

        let at = parse_proxy("u:p@1.2.3.4:8080").unwrap();
        assert_eq!(at.host_port, "1.2.3.4:8080");
        assert_eq!(at.auth_b64.as_deref(), Some("dTpw"));

        let colon = parse_proxy("1.2.3.4:8080:u:p").unwrap();
        assert_eq!(colon.host_port, "1.2.3.4:8080");
        assert_eq!(colon.auth_b64.as_deref(), Some("dTpw"));

        let scheme = parse_proxy("http://u:p@1.2.3.4:8080").unwrap();
        assert_eq!(scheme.host_port, "1.2.3.4:8080");
        assert_eq!(scheme.auth_b64.as_deref(), Some("dTpw"));
    }

    #[test]
    fn rejects_blank_socks_and_garbage() {
        assert!(parse_proxy("").is_none());
        assert!(parse_proxy("   ").is_none());
        assert!(parse_proxy("socks5://1.2.3.4:1080").is_none());
        assert!(parse_proxy("justhost").is_none()); // no port
        assert!(parse_proxy("a:b:c").is_none()); // 3 parts — ambiguous
    }
}

