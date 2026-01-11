//! EchoVault Tauri Desktop App
//!
//! Mini window app similar to Google Drive Desktop.
//! Features:
//! - System tray with menu
//! - Periodic background sync
//! - Notifications when sync completes
//! - Autostart on login
//! - Auto-update on startup

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_updater::UpdaterExt;

mod commands;

/// Check for updates on app startup.
/// If an update is available, prompt the user and install if accepted.
async fn check_for_updates(app: AppHandle) {
    tracing::info!("Checking for updates...");

    // Build and check for updates
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(e) => {
            tracing::warn!("Cannot create updater: {}", e);
            return;
        }
    };

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            tracing::info!("App is up to date");
            return;
        }
        Err(e) => {
            tracing::warn!("Update check failed: {}", e);
            return;
        }
    };

    tracing::info!(
        "Update available: {} -> {}",
        update.current_version,
        update.version
    );

    // Show notification about the update
    // Note: with "dialog": true in tauri.conf.json, the plugin will show
    // a native dialog asking user to confirm before downloading.
    // Here we just log and let the dialog handle confirmation.

    // Download and install the update
    let mut downloaded: usize = 0;

    match update
        .download_and_install(
            |chunk, content_length| {
                downloaded += chunk;
                if let Some(total) = content_length {
                    tracing::debug!("Downloaded {} / {} bytes", downloaded, total);
                }
            },
            || {
                tracing::info!("Download complete, preparing to install...");
            },
        )
        .await
    {
        Ok(_) => {
            tracing::info!("Update installed successfully, app will restart");
            // The app will be restarted automatically
        }
        Err(e) => {
            tracing::error!("Update installation failed: {}", e);
        }
    }
}

/// Setup system tray with menu.
/// Uses a dynamic toggle item that changes between Show/Hide based on window state.
/// On Linux, click events are not supported (AppIndicator protocol limitation),
/// so the menu-based toggle is the primary way to show/hide the window.
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    // Toggle item with label "Show/Hide Window" - action depends on current visibility
    let toggle = MenuItem::with_id(app, "toggle", "Show/Hide Window", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&toggle, &quit])?;

    // Use unique ID to avoid collision with other Tauri apps on Linux
    let _tray = TrayIconBuilder::with_id("com.n24q02m.echovault")
        .menu(&menu)
        .tooltip("EchoVault")
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => {
                if let Some(window) = app.get_webview_window("main") {
                    // Toggle based on current visibility state
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Note: Linux does not support tray icon click events (AppIndicator limitation)
            // This handler only works on Windows and macOS
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("echovault=info".parse().unwrap())
                .add_directive("echovault_core=info".parse().unwrap())
                .add_directive("echovault_lib=info".parse().unwrap()),
        )
        .with_target(true)
        .init();

    tracing::info!("EchoVault starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(commands::AppState::default())
        .setup(|app| {
            setup_tray(app)?;

            // Spawn background task to check for updates
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                check_for_updates(handle).await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Minimize to tray instead of closing completely
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_setup_complete,
            commands::complete_setup,
            commands::get_config,
            commands::get_auth_status,
            commands::start_auth,
            commands::complete_auth,
            commands::scan_sessions,
            commands::sync_vault,
            commands::open_url,
            commands::read_file_content,
            // Settings commands
            commands::get_app_info,
            commands::get_autostart_status,
            commands::set_autostart,
            commands::get_export_path,
            commands::set_export_path,
            commands::open_data_folder,
            commands::open_logs_folder,
            commands::check_update_manual,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
