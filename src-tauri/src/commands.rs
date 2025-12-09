//! Tauri commands - API giữa frontend và backend
//!
//! Các commands này được gọi từ frontend qua IPC.
//! Simplified version - only Rclone provider, no encryption.

use echovault_core::{AuthStatus, Config, RcloneProvider, SyncOptions, SyncProvider, VaultMetadata};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;

/// State chứa RcloneProvider
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<Mutex<RcloneProvider>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            provider: Arc::new(Mutex::new(RcloneProvider::new())),
        }
    }
}

/// Thông tin một session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub source: String,
    pub title: Option<String>,
    pub workspace_name: Option<String>,
    pub created_at: Option<String>,
    pub file_size: u64,
    pub path: String,
}

/// Kết quả scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Trạng thái auth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusResponse {
    pub status: String,
    pub message: Option<String>,
}

impl From<AuthStatus> for AuthStatusResponse {
    fn from(status: AuthStatus) -> Self {
        match status {
            AuthStatus::NotAuthenticated => Self {
                status: "not_authenticated".to_string(),
                message: None,
            },
            AuthStatus::Authenticated => Self {
                status: "authenticated".to_string(),
                message: Some("Connected to cloud storage".to_string()),
            },
            AuthStatus::Pending { user_code, verify_url } => Self {
                status: "pending".to_string(),
                message: Some(format!("Enter code {} at {}", user_code, verify_url)),
            },
            AuthStatus::Error(message) => Self {
                status: "error".to_string(),
                message: Some(message),
            },
        }
    }
}

/// Config để frontend hiển thị
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub setup_complete: bool,
    pub vault_path: String,
    pub remote_name: Option<String>,
    pub folder_name: String,
}

/// Setup request từ frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupRequest {
    pub folder_name: String,
}

// ============ SETUP COMMANDS ============

/// Kiểm tra đã setup chưa
#[tauri::command]
pub async fn check_setup_complete() -> Result<bool, String> {
    let config = Config::load_default().map_err(|e| e.to_string())?;
    Ok(config.setup_complete)
}

/// Hoàn tất setup
#[tauri::command]
pub async fn complete_setup(
    request: SetupRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use echovault_core::config::default_config_path;

    println!("[complete_setup] Starting setup with folder: {}", request.folder_name);

    let mut config = Config::load_default().map_err(|e| e.to_string())?;
    let vault_path = config.vault_path.clone();

    // Check if Rclone remote is configured
    let provider = state.provider.lock().map_err(|e| e.to_string())?;
    if !provider.check_remote_exists().unwrap_or(false) {
        return Err("Please connect to cloud storage first".to_string());
    }
    drop(provider);

    // Update config
    config.setup_complete = true;
    config.sync.remote_name = Some("echovault".to_string());
    config.sync.folder_name = request.folder_name;

    config
        .save(&default_config_path())
        .map_err(|e| e.to_string())?;

    // Create vault directory if not exists
    if !vault_path.exists() {
        std::fs::create_dir_all(&vault_path)
            .map_err(|e| format!("Failed to create vault directory: {}", e))?;
        println!("[complete_setup] Vault directory created: {:?}", vault_path);
    }

    // Create vault metadata
    if !VaultMetadata::exists(&vault_path) {
        let metadata = VaultMetadata::new();
        metadata
            .save(&vault_path)
            .map_err(|e| format!("Failed to save vault metadata: {}", e))?;
        println!("[complete_setup] vault.json created");
    }

    println!("[complete_setup] Setup complete!");
    Ok(())
}

// ============ CONFIG COMMANDS ============

/// Lấy config hiện tại
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    let config = Config::load_default().map_err(|e| e.to_string())?;
    Ok(AppConfig {
        setup_complete: config.setup_complete,
        vault_path: config.vault_path.to_string_lossy().to_string(),
        remote_name: config.sync.remote_name,
        folder_name: config.sync.folder_name,
    })
}

// ============ AUTH COMMANDS ============

/// Lấy trạng thái auth hiện tại
#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    let provider = state.provider.lock().map_err(|e| e.to_string())?;
    Ok(AuthStatusResponse::from(provider.auth_status()))
}

/// Bắt đầu auth flow - mở browser để user đăng nhập
#[tauri::command]
pub async fn start_auth(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    let mut provider = state.provider.lock().map_err(|e| e.to_string())?;
    let status = provider.start_auth().map_err(|e| e.to_string())?;
    Ok(AuthStatusResponse::from(status))
}

/// Hoàn tất auth - check xem user đã đăng nhập chưa
#[tauri::command]
pub async fn complete_auth(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    let mut provider = state.provider.lock().map_err(|e| e.to_string())?;
    let status = provider.complete_auth().map_err(|e| e.to_string())?;
    Ok(AuthStatusResponse::from(status))
}

// ============ SESSION COMMANDS ============

/// Scan tất cả sessions có sẵn
#[tauri::command]
pub async fn scan_sessions() -> Result<ScanResult, String> {
    use echovault_core::extractors::{vscode_copilot::VSCodeCopilotExtractor, Extractor};

    let sessions = tokio::task::spawn_blocking(move || {
        let mut all_sessions = Vec::new();

        // Use VSCodeCopilotExtractor
        let extractor = VSCodeCopilotExtractor::new();
        
        match extractor.find_storage_locations() {
            Ok(locations) => {
                for location in locations {
                    match extractor.list_session_files(&location) {
                        Ok(files) => {
                            for file in files {
                                all_sessions.push(SessionInfo {
                                    id: file.metadata.id,
                                    source: file.metadata.source,
                                    title: file.metadata.title,
                                    workspace_name: file.metadata.workspace_name,
                                    created_at: file.metadata.created_at.map(|d| d.to_rfc3339()),
                                    file_size: file.metadata.file_size,
                                    path: file.source_path.to_string_lossy().to_string(),
                                });
                            }
                        }
                        Err(e) => {
                            eprintln!("Error listing sessions in {:?}: {}", location, e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error finding storage locations: {}", e);
            }
        }

        all_sessions
    })
    .await
    .map_err(|e| e.to_string())?;

    let total = sessions.len();
    Ok(ScanResult { sessions, total })
}

// ============ SYNC COMMANDS ============

/// Sync vault với cloud
#[tauri::command]
pub async fn sync_vault(state: State<'_, AppState>) -> Result<String, String> {
    let config = Config::load_default().map_err(|e| e.to_string())?;
    let vault_path = config.vault_path.clone();

    let provider = state.provider.clone();

    let result = tokio::task::spawn_blocking(move || {
        let provider = provider.lock().map_err(|e| e.to_string())?;

        // Push local changes to cloud
        let options = SyncOptions::default();
        let push_result = provider.push(&vault_path, &options).map_err(|e| e.to_string())?;

        Ok::<_, String>(format!(
            "Synced {} files",
            push_result.files_pushed
        ))
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(result)
}

// ============ UTILITY COMMANDS ============

/// Mở URL trong browser
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Mở file trong file manager
#[tauri::command]
pub async fn open_file(path: String) -> Result<(), String> {
    let path = std::path::Path::new(&path);

    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
