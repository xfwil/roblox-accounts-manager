mod commands;
mod crypto;
mod error;
mod models;
mod nexus;
mod roblox;
mod state;
mod storage;
mod watcher;
mod webapi;

use commands::SettingsPath;
use state::AppState;
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Hold the Roblox singleton mutexes to allow multiple Roblox instances.
/// Roblox uses TWO named mutexes to prevent multiple instances:
/// - "ROBLOX_singletonMutex" (legacy, still checked)
/// - "ROBLOX_singletonEvent" (modern, primary singleton check since ~2024)
/// By acquiring both before Roblox launches, we bypass the restriction.
#[cfg(target_os = "windows")]
fn acquire_multi_roblox_mutex() -> bool {
    unsafe {
        let mut success = true;

        // Acquire legacy mutex
        let name1: Vec<u16> = "ROBLOX_singletonMutex\0".encode_utf16().collect();
        let handle1 = windows_sys::Win32::System::Threading::CreateMutexW(
            std::ptr::null(),
            1, // bInitialOwner = TRUE
            name1.as_ptr(),
        );
        if handle1.is_null() {
            log::warn!("Failed to acquire ROBLOX_singletonMutex");
            success = false;
        } else {
            // Intentionally leak — must be held for the app's lifetime
            log::info!("Acquired ROBLOX_singletonMutex");
        }

        // Acquire modern singleton event mutex
        let name2: Vec<u16> = "ROBLOX_singletonEvent\0".encode_utf16().collect();
        let handle2 = windows_sys::Win32::System::Threading::CreateMutexW(
            std::ptr::null(),
            1, // bInitialOwner = TRUE
            name2.as_ptr(),
        );
        if handle2.is_null() {
            log::warn!("Failed to acquire ROBLOX_singletonEvent");
            success = false;
        } else {
            // Intentionally leak — must be held for the app's lifetime
            log::info!("Acquired ROBLOX_singletonEvent");
        }

        if success {
            log::info!("Multi-Roblox enabled (both mutexes acquired)");
        } else {
            log::warn!("Multi-Roblox may not work — not all mutexes acquired");
        }

        success
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Always enable logging (both debug and release) — writes to app log dir
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;

            // Initialize data directory
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            std::fs::create_dir_all(&app_data_dir)
                .expect("Failed to create app data directory");

            // Generate or load encryption key
            let key_path = app_data_dir.join("key.bin");
            let encryption_key = if key_path.exists() {
                let key_bytes = std::fs::read(&key_path)
                    .expect("Failed to read encryption key");
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes[..32]);
                key
            } else {
                let key = crypto::generate_key();
                std::fs::write(&key_path, &key)
                    .expect("Failed to write encryption key");
                key
            };

            // Initialize account store (shared via Arc for web API + Nexus)
            let store = storage::AccountStore::new(app_data_dir.clone(), encryption_key)
                .expect("Failed to initialize account store");
            let store = Arc::new(Mutex::new(store));

            // Load settings
            let settings_path = app_data_dir.join("settings.json");
            let settings = if settings_path.exists() {
                let data = std::fs::read_to_string(&settings_path)
                    .unwrap_or_default();
                serde_json::from_str::<commands::AppSettings>(&data)
                    .unwrap_or_default()
            } else {
                commands::AppSettings::default()
            };

            // Manage settings path
            app.manage(SettingsPath(settings_path));

            // Manage join data path (presets + recent history)
            let join_data_path = app_data_dir.join("join_data.json");
            app.manage(commands::JoinDataPath(join_data_path));

            // Initialize watcher
            let watcher_state = watcher::new_watcher();
            app.manage(watcher_state);

            // Start Web API server if enabled
            if settings.api_port > 0 {
                let api_store = store.clone();
                let api_port = settings.api_port;
                tauri::async_runtime::spawn(async move {
                    webapi::start_web_api(api_store, api_port).await;
                });
            }

            // Start Nexus WebSocket server if enabled
            if settings.nexus_port > 0 {
                let nexus_store = store.clone();
                let nexus_port = settings.nexus_port;
                tauri::async_runtime::spawn(async move {
                    nexus::start_nexus(nexus_store, nexus_port).await;
                });
            }

            // Acquire Roblox singleton mutex for multi-instance support.
            // We intentionally never release this — it must be held for the app's lifetime.
            #[cfg(target_os = "windows")]
            {
                let _ = acquire_multi_roblox_mutex();
            }

            // Manage app state
            app.manage(AppState {
                store,
                roblox: roblox::RobloxClient::new(),
                join_throttle: std::sync::Arc::new(tokio::sync::Mutex::new(
                    state::JoinThrottle::new(),
                )),
            });

            // Set up system tray
            setup_tray(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Account CRUD
            commands::list_accounts,
            commands::get_account,
            commands::add_account,
            commands::remove_account,
            commands::update_account,
            commands::refresh_account,
            commands::reorder_accounts,
            commands::get_groups,
            commands::get_account_count,
            // Game launching & servers
            commands::join_game,
            commands::get_servers,
            commands::get_game_details,
            commands::get_roblox_path,
            commands::resolve_private_server,
            commands::resolve_share_link,
            // Account utilities
            commands::change_password,
            commands::change_email,
            commands::get_privacy_settings,
            commands::set_account_description,
            commands::set_display_name,
            commands::get_gender,
            commands::get_birthdate,
            // Import/Export
            commands::import_cookies,
            commands::import_user_pass_cookie,
            commands::export_accounts,
            commands::get_account_cookie,
            commands::update_account_cookie,
            // Watcher / Process management
            commands::track_process,
            commands::untrack_process,
            commands::get_process_status,
            commands::cleanup_dead_processes,
            commands::get_running_count,
            commands::find_roblox_processes,

            // Settings
            commands::get_settings,
            commands::save_settings,
            commands::get_themes,
            // Presets & Recent History
            commands::get_recent_place_ids,
            commands::get_recent_ps_links,
            commands::add_recent_place_id,
            commands::add_recent_ps_link,
            commands::remove_recent_place_id,
            commands::remove_recent_ps_link,
            commands::get_presets,
            commands::add_preset,
            commands::update_preset,
            commands::remove_preset,
            // Login & Browser
            commands::open_login_window,
            commands::open_browser,
            // Presence
            commands::get_account_presences,
            commands::get_single_presence,
            // Legacy import
            commands::import_legacy_accounts,

        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show)
        .separator()
        .item(&quit)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .tooltip("Roblox Account Manager")
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

