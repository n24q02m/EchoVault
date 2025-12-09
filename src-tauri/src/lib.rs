//! EchoVault Tauri Desktop App
//!
//! Mini window app giống Google Drive Desktop.
//! Features:
//! - System tray với menu
//! - Background sync định kỳ
//! - Notifications khi sync xong
//! - Autostart khi login

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

mod commands;

/// Tạo system tray với menu
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
    let sync_now = MenuItem::with_id(app, "sync_now", "Đồng bộ ngay", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Cài đặt", true, None::<&str>)?;
    let open_vault = MenuItem::with_id(app, "open_vault", "Mở Vault", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&sync_now, &open_vault, &settings, &quit])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            "sync_now" => {
                // Emit event để frontend trigger sync
                let _ = app.emit("trigger-sync", ());
                // Hiển thị window nếu đang ẩn
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "open_vault" => {
                // Open vault directory trong file manager
                if let Ok(config) = echovault_core::Config::load_default() {
                    #[cfg(target_os = "linux")]
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&config.vault_path)
                        .spawn();
                    // NOTE: macOS support temporarily disabled
                    // #[cfg(target_os = "macos")]
                    // let _ = std::process::Command::new("open")
                    //     .arg(&config.vault_path)
                    //     .spawn();
                    #[cfg(target_os = "windows")]
                    let _ = std::process::Command::new("explorer")
                        .arg(&config.vault_path)
                        .spawn();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
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
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(commands::AppState::default())
        .setup(|app| {
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Minimize to tray thay vì đóng hoàn toàn
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Ẩn window thay vì đóng
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_setup_complete,
            commands::complete_setup,
            commands::check_repo_exists,
            commands::clone_vault,
            commands::get_vault_metadata,
            commands::verify_passphrase_cmd,
            commands::init_provider,
            commands::get_auth_status,
            commands::start_auth,
            commands::complete_auth,
            commands::scan_sessions,
            commands::open_url,
            commands::open_file,
            commands::get_config,
            commands::sync_vault,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
