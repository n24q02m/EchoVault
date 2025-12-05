//! VS Code Copilot Extractor
//!
//! Trích xuất chat history từ GitHub Copilot trong VS Code.
//! CHỈ COPY raw JSON files, KHÔNG parse/transform nội dung.

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// VS Code Copilot Extractor
pub struct VSCodeCopilotExtractor {
    /// Các đường dẫn có thể chứa workspaceStorage
    storage_paths: Vec<PathBuf>,
}

impl VSCodeCopilotExtractor {
    /// Tạo extractor mới với các đường dẫn mặc định theo platform
    pub fn new() -> Self {
        let mut storage_paths = Vec::new();

        // Lấy đường dẫn theo platform
        if let Some(config_dir) = dirs::config_dir() {
            // Linux: ~/.config/Code/User/workspaceStorage
            // macOS: ~/Library/Application Support/Code/User/workspaceStorage
            storage_paths.push(config_dir.join("Code/User/workspaceStorage"));
            storage_paths.push(config_dir.join("Code - Insiders/User/workspaceStorage"));
        }

        #[cfg(target_os = "windows")]
        if let Some(appdata) = dirs::data_dir() {
            // Windows: %APPDATA%\Code\User\workspaceStorage
            storage_paths.push(appdata.join("Code/User/workspaceStorage"));
            storage_paths.push(appdata.join("Code - Insiders/User/workspaceStorage"));
        }

        // WSL: ~/.vscode-server/data/User/workspaceStorage
        if let Some(home) = dirs::home_dir() {
            storage_paths.push(home.join(".vscode-server/data/User/workspaceStorage"));
        }

        // WSL: Truy cập Windows filesystem qua /mnt/c
        #[cfg(target_os = "linux")]
        {
            if std::path::Path::new("/mnt/c/Windows").exists() {
                if let Ok(entries) = std::fs::read_dir("/mnt/c/Users") {
                    for entry in entries.flatten() {
                        let username = entry.file_name();
                        let username_str = username.to_string_lossy();
                        // Bỏ qua các thư mục hệ thống
                        if username_str == "Public"
                            || username_str == "Default"
                            || username_str == "Default User"
                            || username_str == "All Users"
                        {
                            continue;
                        }
                        let appdata_path = entry.path().join("AppData/Roaming");
                        if appdata_path.exists() {
                            storage_paths.push(appdata_path.join("Code/User/workspaceStorage"));
                            storage_paths
                                .push(appdata_path.join("Code - Insiders/User/workspaceStorage"));
                        }
                    }
                }
            }
        }

        Self { storage_paths }
    }

    /// Extract metadata nhanh từ JSON file (chỉ đọc fields cần thiết)
    fn extract_quick_metadata(
        &self,
        path: &PathBuf,
        workspace_name: &str,
    ) -> Option<SessionMetadata> {
        let content = std::fs::read_to_string(path).ok()?;
        let json: Value = serde_json::from_str(&content).ok()?;

        // Lấy session ID từ filename hoặc JSON
        let session_id = json
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            });

        // Lấy title nếu có
        let title = json
            .get("customTitle")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                // Fallback: lấy text từ request đầu tiên
                json.get("requests")
                    .and_then(|r| r.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|req| req.get("message"))
                    .and_then(|msg| msg.get("text"))
                    .and_then(|t| t.as_str())
                    .map(|s| {
                        // Truncate title
                        let truncated: String = s.chars().take(60).collect();
                        if s.chars().count() > 60 {
                            format!("{}...", truncated)
                        } else {
                            truncated
                        }
                    })
            });

        // Lấy timestamp
        let created_at = json
            .get("creationDate")
            .and_then(|v| v.as_i64())
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        // Lấy file size
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        Some(SessionMetadata {
            id: session_id,
            source: "vscode-copilot".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(), // Sẽ được set sau khi copy
            original_path: path.clone(),
            file_size,
            workspace_name: Some(workspace_name.to_string()),
        })
    }
}

impl Default for VSCodeCopilotExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for VSCodeCopilotExtractor {
    fn source_name(&self) -> &'static str {
        "vscode-copilot"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let mut workspaces = Vec::new();

        for storage_path in &self.storage_paths {
            if !storage_path.exists() {
                continue;
            }

            // Duyệt qua tất cả workspace hash directories
            if let Ok(entries) = std::fs::read_dir(storage_path) {
                for entry in entries.flatten() {
                    let chat_sessions_dir = entry.path().join("chatSessions");
                    if chat_sessions_dir.exists() && chat_sessions_dir.is_dir() {
                        // Kiểm tra có file JSON nào không
                        if let Ok(sessions) = std::fs::read_dir(&chat_sessions_dir) {
                            let has_json = sessions
                                .flatten()
                                .any(|e| e.path().extension().is_some_and(|ext| ext == "json"));
                            if has_json {
                                workspaces.push(entry.path());
                            }
                        }
                    }
                }
            }
        }

        Ok(workspaces)
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        let workspace_json = location.join("workspace.json");
        if workspace_json.exists() {
            if let Ok(content) = std::fs::read_to_string(&workspace_json) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    return json
                        .get("folder")
                        .and_then(|v| v.as_str())
                        .map(|s| {
                            // Lấy tên folder cuối cùng từ URI
                            s.rsplit('/').next().unwrap_or(s).to_string()
                        })
                        .unwrap_or_else(|| "Unknown".to_string());
                }
            }
        }
        "Unknown".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let chat_sessions_dir = location.join("chatSessions");
        if !chat_sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let workspace_name = self.get_workspace_name(location);
        let mut sessions = Vec::new();

        for entry in std::fs::read_dir(&chat_sessions_dir)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                // Extract metadata nhanh
                if let Some(metadata) = self.extract_quick_metadata(&path, &workspace_name) {
                    let session_id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    sessions.push(SessionFile {
                        source_path: path,
                        session_id,
                        metadata,
                    });
                }
            }
        }

        // Sắp xếp theo thời gian tạo (mới nhất trước)
        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

        Ok(sessions)
    }
}
