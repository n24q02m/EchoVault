//! Tauri commands - API giữa frontend và backend
//!
//! Các commands này được gọi từ frontend qua IPC.

use echovault_core::{AuthStatus, GitHubProvider, SyncOptions, SyncProvider};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;

/// State chứa provider hiện tại
#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<Mutex<GitHubProvider>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            provider: Arc::new(Mutex::new(GitHubProvider::new())),
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
}

/// Kết quả scan - grouped theo source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Trạng thái auth để frontend hiển thị
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusResponse {
    pub status: String, // "not_authenticated", "pending", "authenticated", "error"
    pub user_code: Option<String>,
    pub verify_url: Option<String>,
    pub error: Option<String>,
}

impl From<AuthStatus> for AuthStatusResponse {
    fn from(status: AuthStatus) -> Self {
        match status {
            AuthStatus::NotAuthenticated => Self {
                status: "not_authenticated".to_string(),
                user_code: None,
                verify_url: None,
                error: None,
            },
            AuthStatus::Pending {
                user_code,
                verify_url,
            } => Self {
                status: "pending".to_string(),
                user_code: Some(user_code),
                verify_url: Some(verify_url),
                error: None,
            },
            AuthStatus::Authenticated => Self {
                status: "authenticated".to_string(),
                user_code: None,
                verify_url: None,
                error: None,
            },
            AuthStatus::Error(e) => Self {
                status: "error".to_string(),
                user_code: None,
                verify_url: None,
                error: Some(e),
            },
        }
    }
}

/// Config để frontend hiển thị
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub setup_complete: bool,
    pub provider: String,
    pub repo_name: Option<String>,
    pub encrypt: bool,
    pub compress: bool,
}

/// Setup request từ frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupRequest {
    pub provider: String,
    pub repo_name: String,
    pub encrypt: bool,
    pub compress: bool,
    pub passphrase: Option<String>,
}

// ============ SETUP COMMANDS ============

/// Kiểm tra đã setup chưa
#[tauri::command]
pub async fn check_setup_complete() -> Result<bool, String> {
    use echovault_core::Config;
    let config = Config::load_default().map_err(|e| e.to_string())?;
    Ok(config.setup_complete)
}

/// Hoàn tất setup
#[tauri::command]
pub async fn complete_setup(
    request: SetupRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use echovault_core::config::{
        default_config_path, CompressionConfig, EncryptionConfig, SyncConfig,
    };
    use echovault_core::Config;

    let provider = state.provider.lock().map_err(|e| e.to_string())?;

    // Lấy username từ GitHub nếu đã auth
    let username = if provider.is_authenticated() {
        // TODO: Get username from GitHub API
        "user".to_string()
    } else {
        "user".to_string()
    };

    // Tạo remote URL
    let remote_url = format!("https://github.com/{}/{}.git", username, request.repo_name);

    let mut config = Config::load_default().map_err(|e| e.to_string())?;
    config.setup_complete = true;
    config.sync = SyncConfig {
        remote: Some(remote_url),
        repo_name: Some(request.repo_name),
        provider: request.provider,
    };
    config.encryption = EncryptionConfig {
        enabled: request.encrypt,
    };
    config.compression = CompressionConfig {
        enabled: request.compress,
    };

    config
        .save(&default_config_path())
        .map_err(|e| e.to_string())?;

    // Lưu passphrase vào keyring nếu encryption enabled
    if request.encrypt {
        if let Some(passphrase) = request.passphrase {
            let entry = keyring::Entry::new("echovault", "passphrase")
                .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
            entry
                .set_password(&passphrase)
                .map_err(|e| format!("Failed to save passphrase to keyring: {}", e))?;
        }
    }

    Ok(())
}

// ============ VAULT COMMANDS ============

/// Response cho vault metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMetadataResponse {
    pub exists: bool,
    pub encrypted: bool,
    pub compressed: bool,
}

/// Kiểm tra repo có tồn tại trên GitHub không
#[tauri::command]
pub async fn check_repo_exists(
    repo_name: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let provider = state.provider.clone();

    let exists = tokio::task::spawn_blocking(move || {
        let provider = provider.lock().map_err(|e| e.to_string())?;
        // Sử dụng GitHub API để check
        provider.repo_exists(&repo_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: String| e)?;

    Ok(exists)
}

/// Clone vault từ remote về local
#[tauri::command]
pub async fn clone_vault(repo_name: String, state: State<'_, AppState>) -> Result<(), String> {
    let provider = state.provider.clone();

    tokio::task::spawn_blocking(move || {
        let provider = provider.lock().map_err(|e| e.to_string())?;
        provider.clone_repo(&repo_name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: String| e)?;

    Ok(())
}

/// Đọc vault metadata sau khi clone
#[tauri::command]
pub async fn get_vault_metadata() -> Result<VaultMetadataResponse, String> {
    use echovault_core::config::default_vault_path;
    use echovault_core::VaultMetadata;

    let vault_path = default_vault_path();

    if !VaultMetadata::exists(&vault_path) {
        return Ok(VaultMetadataResponse {
            exists: false,
            encrypted: false,
            compressed: false,
        });
    }

    let metadata = VaultMetadata::load(&vault_path).map_err(|e| e.to_string())?;

    Ok(VaultMetadataResponse {
        exists: true,
        encrypted: metadata.encrypted,
        compressed: metadata.compressed,
    })
}

/// Verify passphrase cho encrypted vault
#[tauri::command]
pub async fn verify_passphrase_cmd(passphrase: String) -> Result<bool, String> {
    use echovault_core::config::default_vault_path;

    let vault_path = default_vault_path();

    tokio::task::spawn_blocking(move || {
        echovault_core::verify_passphrase(&vault_path, &passphrase).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ============ AUTH COMMANDS ============

/// Lấy trạng thái auth hiện tại
#[tauri::command]
pub async fn get_auth_status(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    let provider = state.provider.lock().map_err(|e| e.to_string())?;
    Ok(provider.auth_status().into())
}

/// Bắt đầu auth flow - trả về user_code và verify_url
#[tauri::command]
pub async fn start_auth(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    // Clone provider to use in spawn_blocking
    let provider = state.provider.clone();

    let status = tokio::task::spawn_blocking(move || {
        let mut provider = provider.lock().map_err(|e| e.to_string())?;
        provider.start_auth().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: String| e)?;

    Ok(status.into())
}

/// Hoàn tất auth - poll cho token
#[tauri::command]
pub async fn complete_auth(state: State<'_, AppState>) -> Result<AuthStatusResponse, String> {
    // Clone provider to use in spawn_blocking
    let provider = state.provider.clone();

    let status = tokio::task::spawn_blocking(move || {
        let mut provider = provider.lock().map_err(|e| e.to_string())?;
        provider.complete_auth().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: String| e)?;

    Ok(status.into())
}

// ============ SESSION COMMANDS ============

/// Scan tất cả sessions có sẵn
#[tauri::command]
pub async fn scan_sessions() -> Result<ScanResult, String> {
    use echovault_core::extractors::{vscode_copilot::VSCodeCopilotExtractor, Extractor};

    let extractor = VSCodeCopilotExtractor::new();
    let locations = extractor
        .find_storage_locations()
        .map_err(|e| e.to_string())?;

    let mut sessions = Vec::new();

    for location in locations {
        if let Ok(files) = extractor.list_session_files(&location) {
            for file in files {
                sessions.push(SessionInfo {
                    id: file.metadata.id,
                    source: file.metadata.source,
                    title: file.metadata.title,
                    workspace_name: file.metadata.workspace_name,
                    created_at: file.metadata.created_at.map(|d| d.to_rfc3339()),
                    file_size: file.metadata.file_size,
                });
            }
        }
    }

    // Sort by created_at descending (newest first)
    sessions.sort_by(|a, b| {
        let a_time = a.created_at.as_ref().map(|s| s.as_str()).unwrap_or("");
        let b_time = b.created_at.as_ref().map(|s| s.as_str()).unwrap_or("");
        b_time.cmp(a_time)
    });

    let total = sessions.len();
    Ok(ScanResult { sessions, total })
}

/// Mở URL trong browser (hỗ trợ WSL)
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    use std::process::Command;

    // Kiểm tra nếu đang chạy trong WSL
    let is_wsl = std::fs::read_to_string("/proc/version")
        .map(|v| v.contains("microsoft") || v.contains("WSL"))
        .unwrap_or(false);

    if is_wsl {
        // Trong WSL, dùng cmd.exe để mở browser trong Windows
        Command::new("cmd.exe")
            .args(["/c", "start", "", &url])
            .spawn()
            .map_err(|e| format!("Failed to open URL in Windows: {}", e))?;
    } else {
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg(&url)
                .spawn()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "windows")]
        {
            Command::new("cmd")
                .args(["/c", "start", "", &url])
                .spawn()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "macos")]
        {
            Command::new("open")
                .arg(&url)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Mở file trong text editor
#[tauri::command]
pub async fn open_file(file_path: String) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("notepad")
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-t")
            .arg(&file_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ============ CONFIG COMMANDS ============

/// Lấy config hiện tại
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    use echovault_core::Config;

    let config = Config::load_default().map_err(|e| e.to_string())?;

    Ok(AppConfig {
        setup_complete: config.setup_complete,
        provider: config.sync.provider,
        repo_name: config.sync.repo_name,
        encrypt: config.encryption.enabled,
        compress: config.compression.enabled,
    })
}

// ============ SYNC COMMANDS ============

/// Sync vault lên remote (sẽ gọi tự động, không cần manual)
#[tauri::command]
pub async fn sync_vault(state: State<'_, AppState>) -> Result<bool, String> {
    use echovault_core::Config;

    let provider = state.provider.lock().map_err(|e| e.to_string())?;

    if !provider.is_authenticated() {
        return Err("Not authenticated".to_string());
    }

    let config = Config::load_default().map_err(|e| e.to_string())?;
    let vault_dir = config.vault_path;

    let options = SyncOptions {
        encrypt: config.encryption.enabled,
        compress: config.compression.enabled,
    };

    let result = provider
        .push(&vault_dir, &options)
        .map_err(|e| e.to_string())?;

    Ok(result.success)
}
