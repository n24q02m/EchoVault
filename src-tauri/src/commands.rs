//! Tauri commands - API giữa frontend và backend
//!
//! Các commands này được gọi từ frontend qua IPC.

use serde::{Deserialize, Serialize};

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

/// Kết quả scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub sessions: Vec<SessionInfo>,
    pub total: usize,
}

/// Kết quả sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub success: bool,
    pub message: String,
    pub extracted_count: usize,
    pub encrypted_count: usize,
}

/// Config đơn giản để frontend hiển thị
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub remote: Option<String>,
    pub encrypt: bool,
    pub compress: bool,
    pub auto_sync_minutes: u32,
    pub provider: String,
}

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

    let total = sessions.len();
    Ok(ScanResult { sessions, total })
}

/// Sync vault lên remote
#[tauri::command]
pub async fn sync_vault() -> Result<SyncResult, String> {
    // TODO: Implement using echovault-core
    // Tạm thời trả về mock data
    Ok(SyncResult {
        success: true,
        message: "Sync completed".to_string(),
        extracted_count: 0,
        encrypted_count: 0,
    })
}

/// Lấy config hiện tại
#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    use echovault_core::Config;

    let config = Config::load_default().map_err(|e| e.to_string())?;

    Ok(AppConfig {
        remote: config.sync.remote,
        encrypt: config.encryption.enabled,
        compress: true, // TODO: Add to config
        auto_sync_minutes: 30,
        provider: "github".to_string(),
    })
}

/// Cập nhật config
#[tauri::command]
pub async fn set_config(config: AppConfig) -> Result<(), String> {
    use echovault_core::config::default_config_path;
    use echovault_core::Config;

    let mut current = Config::load_default().map_err(|e| e.to_string())?;

    if let Some(remote) = config.remote {
        current.set_remote(remote);
    }

    current
        .save(&default_config_path())
        .map_err(|e| e.to_string())?;

    Ok(())
}
