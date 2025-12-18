//! Cline (Claude Dev) VS Code Extension Extractor
//!
//! Trích xuất task history từ Cline extension.
//! CHỈ COPY raw JSON files, KHÔNG parse/transform nội dung.
//!
//! Storage locations:
//! - Windows: %APPDATA%/Code/User/globalStorage/saoudrizwan.claude-dev/tasks
//! - macOS: ~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/tasks
//! - Linux: ~/.config/Code/User/globalStorage/saoudrizwan.claude-dev/tasks

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Cline VS Code Extension Extractor
pub struct ClineExtractor {
    /// Các đường dẫn có thể chứa globalStorage
    storage_paths: Vec<PathBuf>,
}

impl ClineExtractor {
    /// Tạo extractor mới với các đường dẫn mặc định theo platform
    pub fn new() -> Self {
        let mut storage_paths = Vec::new();

        // Ưu tiên đọc từ HOME env variable
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            // Linux: ~/.config/Code/User/globalStorage/saoudrizwan.claude-dev/tasks
            storage_paths.push(
                home_path.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/tasks"),
            );
            // Code Insiders
            storage_paths.push(
                home_path.join(
                    ".config/Code - Insiders/User/globalStorage/saoudrizwan.claude-dev/tasks",
                ),
            );
        }

        // Fallback: Lấy đường dẫn theo platform qua dirs crate
        if let Some(config_dir) = dirs::config_dir() {
            // macOS: ~/Library/Application Support/Code/User/globalStorage/...
            let cline_path =
                config_dir.join("Code/User/globalStorage/saoudrizwan.claude-dev/tasks");
            if !storage_paths.contains(&cline_path) {
                storage_paths.push(cline_path);
            }

            let cline_insiders =
                config_dir.join("Code - Insiders/User/globalStorage/saoudrizwan.claude-dev/tasks");
            if !storage_paths.contains(&cline_insiders) {
                storage_paths.push(cline_insiders);
            }
        }

        #[cfg(target_os = "windows")]
        if let Some(appdata) = dirs::data_dir() {
            // Windows: %APPDATA%/Code/User/globalStorage/saoudrizwan.claude-dev/tasks
            storage_paths
                .push(appdata.join("Code/User/globalStorage/saoudrizwan.claude-dev/tasks"));
            storage_paths.push(
                appdata.join("Code - Insiders/User/globalStorage/saoudrizwan.claude-dev/tasks"),
            );
        }

        Self { storage_paths }
    }

    /// Extract metadata từ task folder
    fn extract_task_metadata(&self, task_dir: &Path) -> Option<SessionMetadata> {
        // Cline lưu mỗi task trong một folder riêng với các files:
        // - api_conversation_history.json (conversation)
        // - ui_messages.json (UI state)
        let api_history = task_dir.join("api_conversation_history.json");

        // Lấy task ID từ folder name
        let task_id = task_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())?;

        // Đọc file để lấy metadata
        let (title, _created_at) = if api_history.exists() {
            if let Ok(content) = std::fs::read_to_string(&api_history) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    let title = json
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|msg| msg.get("content"))
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|item| item.get("text"))
                        .and_then(|t| t.as_str())
                        .map(|s| {
                            let truncated: String = s.chars().take(60).collect();
                            if s.chars().count() > 60 {
                                format!("{}...", truncated)
                            } else {
                                truncated
                            }
                        });
                    (title, None::<chrono::DateTime<Utc>>)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Lấy file size tổng của folder
        let file_size = std::fs::read_dir(task_dir)
            .ok()?
            .flatten()
            .filter_map(|e| std::fs::metadata(e.path()).ok())
            .map(|m| m.len())
            .sum();

        // Lấy created_at từ folder metadata
        let created_at = std::fs::metadata(task_dir)
            .ok()
            .and_then(|m| m.created().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).single());

        Some(SessionMetadata {
            id: task_id,
            source: "cline".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: task_dir.to_path_buf(),
            file_size,
            workspace_name: None,
        })
    }
}

impl Default for ClineExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for ClineExtractor {
    fn source_name(&self) -> &'static str {
        "cline"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let mut locations = Vec::new();

        for storage_path in &self.storage_paths {
            if storage_path.exists() && storage_path.is_dir() {
                // Mỗi task là một subdirectory
                if let Ok(entries) = std::fs::read_dir(storage_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            // Kiểm tra có file api_conversation_history.json
                            if path.join("api_conversation_history.json").exists() {
                                locations.push(path);
                            }
                        }
                    }
                }
            }
        }

        Ok(locations)
    }

    fn get_workspace_name(&self, _location: &Path) -> String {
        "Cline Tasks".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        // location là task folder
        if let Some(metadata) = self.extract_task_metadata(location) {
            Ok(vec![SessionFile {
                source_path: location.join("api_conversation_history.json"),
                metadata,
            }])
        } else {
            Ok(Vec::new())
        }
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        // Mỗi location là một task, nên count = 1
        if location.join("api_conversation_history.json").exists() {
            Ok(1)
        } else {
            Ok(0)
        }
    }
}
