//! Cursor AI Editor Extractor
//!
//! Extracts chat history from Cursor AI Editor.
//! ONLY COPY raw JSON files, DO NOT parse/transform content.
//!
//! Cursor uses the same format as VS Code Copilot since it's forked from VS Code.
//! Storage locations:
//! - Windows: %APPDATA%\Cursor\User\workspaceStorage
//! - macOS: ~/Library/Application Support/Cursor/User/workspaceStorage
//! - Linux: ~/.config/Cursor/User/workspaceStorage

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use rayon::prelude::*;
use serde_json::Value;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Cursor AI Editor Extractor
pub struct CursorExtractor {
    /// Paths that may contain workspaceStorage
    storage_paths: Vec<PathBuf>,
}

impl CursorExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut storage_paths = Vec::new();

        // Prefer reading from HOME env variable (for testing with HOME override)
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            // Linux: $HOME/.config/Cursor/User/workspaceStorage
            storage_paths.push(home_path.join(".config/Cursor/User/workspaceStorage"));
        }

        // Fallback: Get path per platform via dirs crate
        if let Some(config_dir) = dirs::config_dir() {
            // Linux: ~/.config/Cursor/User/workspaceStorage
            // macOS: ~/Library/Application Support/Cursor/User/workspaceStorage
            let cursor_path = config_dir.join("Cursor/User/workspaceStorage");
            if !storage_paths.contains(&cursor_path) {
                storage_paths.push(cursor_path);
            }
        }

        // NOTE: On Windows, dirs::config_dir() already returns %APPDATA% (Roaming)
        // which is the correct location for Cursor storage.
        // dirs::data_dir() returns %LOCALAPPDATA% (Local) which is NOT where Cursor stores data.

        Self { storage_paths }
    }

    /// Quick metadata extraction from JSON/JSONL file (only read required fields).
    fn extract_quick_metadata(
        &self,
        path: &PathBuf,
        workspace_name: &str,
    ) -> Option<SessionMetadata> {
        let is_jsonl = path.extension().is_some_and(|ext| ext == "jsonl");

        let json = if is_jsonl {
            // JSONL format: first line is kind=0 (session header), data in "v" field
            let file = std::fs::File::open(path).ok()?;
            let reader = std::io::BufReader::new(file);
            let first_line = reader.lines().next()?.ok()?;
            let wrapper: Value = serde_json::from_str(&first_line).ok()?;
            wrapper.get("v")?.clone()
        } else {
            let content = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()?
        };

        // Get session ID from filename or JSON
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

        // Get title if available
        let title = json
            .get("customTitle")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                json.get("requests")
                    .and_then(|r| r.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|req| req.get("message"))
                    .and_then(|msg| msg.get("text"))
                    .and_then(|t| t.as_str())
                    .map(|s| {
                        let truncated: String = s.chars().take(60).collect();
                        if s.chars().count() > 60 {
                            format!("{}...", truncated)
                        } else {
                            truncated
                        }
                    })
            })
            .or_else(|| {
                // Fallback for JSONL: read subsequent lines for first user message
                if !is_jsonl {
                    return None;
                }
                let file = std::fs::File::open(path).ok()?;
                let reader = std::io::BufReader::new(file);
                for line in reader.lines().skip(1).take(20).flatten() {
                    if let Ok(obj) = serde_json::from_str::<Value>(&line) {
                        if obj.get("kind").and_then(|k| k.as_i64()) == Some(1) {
                            if let Some(text) = obj.get("v").and_then(|v| v.as_str()) {
                                if !text.is_empty() && text.len() > 5 {
                                    let truncated: String = text.chars().take(60).collect();
                                    return if text.chars().count() > 60 {
                                        Some(format!("{}...", truncated))
                                    } else {
                                        Some(truncated)
                                    };
                                }
                            }
                        }
                    }
                }
                None
            });

        // Get timestamp
        let created_at = json
            .get("creationDate")
            .and_then(|v| v.as_i64())
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        // Get file size
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        Some(SessionMetadata {
            id: session_id,
            source: "cursor".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
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

            // Iterate through all workspace hash directories
            if let Ok(entries) = std::fs::read_dir(storage_path) {
                for entry in entries.flatten() {
                    let chat_sessions_dir = entry.path().join("chatSessions");
                    if chat_sessions_dir.exists() && chat_sessions_dir.is_dir() {
                        // Check if there are any JSON or JSONL files
                        if let Ok(sessions) = std::fs::read_dir(&chat_sessions_dir) {
                            let has_sessions = sessions.flatten().any(|e| {
                                e.path()
                                    .extension()
                                    .is_some_and(|ext| ext == "json" || ext == "jsonl")
                            });
                            if has_sessions {
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
                            // Get last folder name from URI
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

        // Collect all JSON and JSONL paths
        let json_paths: Vec<PathBuf> = std::fs::read_dir(&chat_sessions_dir)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext == "json" || ext == "jsonl")
            })
            .collect();

        // Extract metadata in parallel with rayon
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

        // Sort by creation time (newest first)
        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let chat_sessions_dir = location.join("chatSessions");
        if !chat_sessions_dir.exists() {
            return Ok(0);
        }

        // Count JSON and JSONL files, don't parse metadata
        let count = std::fs::read_dir(&chat_sessions_dir)?
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "json" || ext == "jsonl")
            })
            .count();

        Ok(count)
    }
}
