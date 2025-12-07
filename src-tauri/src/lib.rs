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
                // TODO: Trigger sync
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "open_vault" => {
                // TODO: Open vault directory
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

/// Setup FileWatcher background thread
/// Emit "file_changed" event khi phát hiện IDE file changes
fn setup_file_watcher(app_handle: tauri::AppHandle) {
    use echovault_core::{get_ide_storage_paths, FileWatcher};
    use std::time::Duration;

    println!("[FileWatcher] Initializing...");

    let paths = get_ide_storage_paths();
    if paths.is_empty() {
        println!("[FileWatcher] No IDE paths found, watcher disabled");
        return;
    }

    let watcher = match FileWatcher::new(paths) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[FileWatcher] Failed to create watcher: {}", e);
            return;
        }
    };

    println!(
        "[FileWatcher] Watching {} paths",
        watcher.watched_paths().len()
    );

    // Poll mỗi 5 giây để check for changes
    // Nhẹ hơn nhiều so với full scan mỗi 5 phút
    loop {
        std::thread::sleep(Duration::from_secs(5));

        if watcher.has_changes() {
            println!("[FileWatcher] Changes detected, emitting event...");

            // Emit event đến frontend
            if let Err(e) = app_handle.emit("file_changed", ()) {
                eprintln!("[FileWatcher] Failed to emit event: {}", e);
            }
        }
    }
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

            // Khởi tạo FileWatcher cho IDE directories
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                setup_file_watcher(app_handle);
            });

            Ok(())
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
