use super::{SessionFile, SessionMetadata};
use crate::utils::wsl;
use anyhow::Result;
use chrono::{TimeZone, Utc};
use rayon::prelude::*;
use serde_json::Value;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Common VS Code-like extractor logic.
pub struct VSCodeCommon {
    pub storage_paths: Vec<PathBuf>,
}

impl VSCodeCommon {
    /// Create new extractor with default paths per platform.
    pub fn new(config_subpaths: &[&str], wsl_subpaths: &[&str]) -> Self {
        let mut storage_paths = Vec::new();

        // 1. Check HOME with wsl_subpaths (mostly for Linux/testing)
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            for subpath in wsl_subpaths {
                storage_paths.push(home_path.join(subpath));
            }
        }

        // 2. Check config_dir with config_subpaths
        if let Some(config_dir) = dirs::config_dir() {
            for subpath in config_subpaths {
                let path = config_dir.join(subpath);
                if !storage_paths.contains(&path) {
                    storage_paths.push(path);
                }
            }
        }

        // 3. WSL Check
        if cfg!(target_os = "windows") {
            for subpath in wsl_subpaths {
                for wsl_path in wsl::find_wsl_paths(subpath) {
                    if !storage_paths.contains(&wsl_path) {
                        storage_paths.push(wsl_path);
                    }
                }
            }
        }

        Self { storage_paths }
    }

    /// Find storage locations (workspaces with chatSessions).
    pub fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
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

    /// Get workspace name from workspace.json.
    pub fn get_workspace_name(location: &Path) -> String {
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

    /// Count sessions in a location.
    pub fn count_sessions(location: &Path) -> Result<usize> {
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

    /// Quick metadata extraction from JSON/JSONL file (only read required fields).
    pub fn extract_quick_metadata(
        path: &PathBuf,
        workspace_name: &str,
        source: &str,
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
            source: source.to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: path.clone(),
            file_size,
            workspace_name: Some(workspace_name.to_string()),
            ide_origin: None,
        })
    }

    /// List session files in a location.
    pub fn list_session_files<F>(
        &self,
        location: &Path,
        extractor_source: &str,
        metadata_extractor: F,
    ) -> Result<Vec<SessionFile>>
    where
        F: Fn(&PathBuf, &str, &str) -> Option<SessionMetadata> + Sync + Send,
    {
        let chat_sessions_dir = location.join("chatSessions");
        if !chat_sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let workspace_name = Self::get_workspace_name(location);

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
                metadata_extractor(path, &workspace_name, extractor_source).map(|metadata| {
                    SessionFile {
                        source_path: path.clone(),
                        metadata,
                    }
                })
            })
            .collect();

        // Sort by creation time (newest first)
        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

        Ok(sessions)
    }
}
