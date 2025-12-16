//! Cursor AI Editor Extractor
//!
//! Trích xuất chat history từ Cursor AI Editor.
//! CHỈ COPY raw JSON files, KHÔNG parse/transform nội dung.
//!
//! Cursor sử dụng cùng format với VS Code Copilot vì được fork từ VS Code.
//! Storage locations:
//! - Windows: %APPDATA%\Cursor\User\workspaceStorage
//! - macOS: ~/Library/Application Support/Cursor/User/workspaceStorage
//! - Linux: ~/.config/Cursor/User/workspaceStorage

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use rayon::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Cursor AI Editor Extractor
pub struct CursorExtractor {
    /// Các đường dẫn có thể chứa workspaceStorage
    storage_paths: Vec<PathBuf>,
}

impl CursorExtractor {
    /// Tạo extractor mới với các đường dẫn mặc định theo platform
    pub fn new() -> Self {
        let mut storage_paths = Vec::new();

        // Ưu tiên đọc từ HOME env variable (để hỗ trợ testing với HOME override)
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            // Linux: $HOME/.config/Cursor/User/workspaceStorage
            storage_paths.push(home_path.join(".config/Cursor/User/workspaceStorage"));
        }

        // Fallback: Lấy đường dẫn theo platform qua dirs crate
        if let Some(config_dir) = dirs::config_dir() {
            // Linux: ~/.config/Cursor/User/workspaceStorage
            // macOS: ~/Library/Application Support/Cursor/User/workspaceStorage
            let cursor_path = config_dir.join("Cursor/User/workspaceStorage");
            if !storage_paths.contains(&cursor_path) {
                storage_paths.push(cursor_path);
            }
        }

        #[cfg(target_os = "windows")]
        if let Some(appdata) = dirs::data_dir() {
            // Windows: %APPDATA%\Cursor\User\workspaceStorage
            storage_paths.push(appdata.join("Cursor/User/workspaceStorage"));
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
            source: "cursor".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(), // Sẽ được set sau khi copy
            original_path: path.clone(),
            file_size,
            workspace_name: Some(workspace_name.to_string()),
        })
    }
}

impl Default for CursorExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for CursorExtractor {
    fn source_name(&self) -> &'static str {
        "cursor"
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

        // Thu thập tất cả JSON paths trước
        let json_paths: Vec<PathBuf> = std::fs::read_dir(&chat_sessions_dir)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();

        // Extract metadata song song với rayon
        let mut sessions: Vec<SessionFile> = json_paths
            .par_iter()
            .filter_map(|path| {
                self.extract_quick_metadata(path, &workspace_name)
                    .map(|metadata| SessionFile {
                        source_path: path.clone(),
                        metadata,
                    })
            })
            .collect();

        // Sắp xếp theo thời gian tạo (mới nhất trước)
        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let chat_sessions_dir = location.join("chatSessions");
        if !chat_sessions_dir.exists() {
            return Ok(0);
        }

        // Chỉ đếm số file JSON, không parse metadata
        let count = std::fs::read_dir(&chat_sessions_dir)?
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .count();

        Ok(count)
    }
}
