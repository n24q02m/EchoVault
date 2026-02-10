//! Gemini CLI Extractor
//!
//! Extracts chat history from Google Gemini CLI (open-source).
//! Conversations are stored as JSON files in ~/.gemini/tmp/<project_hash>/chats/
//!
//! Format: session-<timestamp>-<sessionId>.json containing ConversationRecord:
//! - sessionId, projectHash, startTime, lastUpdated
//! - messages[] with id, timestamp, type (user/gemini/info/error/warning), content
//!
//! Also extracts memory.md from ~/.gemini/memory.md

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Gemini CLI Extractor
pub struct GeminiCliExtractor {
    /// Paths to ~/.gemini/ directories
    gemini_dirs: Vec<PathBuf>,
}

impl GeminiCliExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut gemini_dirs = Vec::new();

        // Prefer $HOME env variable
        if let Ok(home) = std::env::var("HOME") {
            let path = PathBuf::from(home).join(".gemini");
            gemini_dirs.push(path);
        }

        // Fallback: dirs crate
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".gemini");
            if !gemini_dirs.contains(&path) {
                gemini_dirs.push(path);
            }
        }

        Self { gemini_dirs }
    }

    /// Find all chat directories: ~/.gemini/tmp/<hash>/chats/
    fn find_chat_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        for gemini_dir in &self.gemini_dirs {
            let tmp_dir = gemini_dir.join("tmp");
            if !tmp_dir.exists() || !tmp_dir.is_dir() {
                continue;
            }

            // Each subdirectory is a project hash
            if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
                for entry in entries.flatten() {
                    let chats_dir = entry.path().join("chats");
                    if chats_dir.exists() && chats_dir.is_dir() {
                        dirs.push(chats_dir);
                    }
                }
            }
        }

        dirs
    }

    /// Extract metadata from a Gemini CLI session JSON file.
    fn extract_session_metadata(&self, path: &Path, project_dir: &str) -> Option<SessionMetadata> {
        let content = std::fs::read_to_string(path).ok()?;
        let json: Value = serde_json::from_str(&content).ok()?;

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

        // Title: use summary if available, otherwise first user message
        let title = json
            .get("summary")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| {
                let truncated: String = s.chars().take(60).collect();
                if s.chars().count() > 60 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            })
            .or_else(|| {
                // Fallback: first user message content text
                json.get("messages")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| {
                        arr.iter()
                            .find(|msg| msg.get("type").and_then(|t| t.as_str()) == Some("user"))
                    })
                    .and_then(|msg| {
                        // content can be string or structured
                        msg.get("content").and_then(|c| {
                            c.as_str().map(|s| s.to_string()).or_else(|| {
                                // Structured content: look for text parts
                                c.as_array().and_then(|parts| {
                                    parts.iter().find_map(|p| {
                                        p.get("text").and_then(|t| t.as_str()).map(String::from)
                                    })
                                })
                            })
                        })
                    })
                    .map(|s| {
                        let truncated: String = s.chars().take(60).collect();
                        if s.chars().count() > 60 {
                            format!("{}...", truncated)
                        } else {
                            truncated
                        }
                    })
            });

        // Parse startTime (ISO 8601)
        let created_at = json
            .get("startTime")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        // Project name from parent directory name
        // chats/ is inside ~/.gemini/tmp/<project_hash>/
        // We use the hash as workspace name (could be resolved later)
        let workspace_name = if project_dir.is_empty() {
            None
        } else {
            Some(project_dir.to_string())
        };

        Some(SessionMetadata {
            id: session_id,
            source: "gemini-cli".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: path.to_path_buf(),
            file_size,
            workspace_name,
            ide_origin: None,
        })
    }
}

impl Default for GeminiCliExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for GeminiCliExtractor {
    fn source_name(&self) -> &'static str {
        "gemini-cli"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let chat_dirs = self.find_chat_dirs();
        // Only return dirs that have .json files
        Ok(chat_dirs
            .into_iter()
            .filter(|dir| {
                std::fs::read_dir(dir)
                    .map(|entries| {
                        entries
                            .flatten()
                            .any(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                    })
                    .unwrap_or(false)
            })
            .collect())
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        // location is ~/.gemini/tmp/<project_hash>/chats/
        // Get parent to get <project_hash>
        location
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Gemini CLI".to_string())
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let project_dir = self.get_workspace_name(location);

        let json_paths: Vec<PathBuf> = std::fs::read_dir(location)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();

        let mut sessions: Vec<SessionFile> = json_paths
            .par_iter()
            .filter_map(|path| {
                self.extract_session_metadata(path, &project_dir)
                    .map(|metadata| SessionFile {
                        source_path: path.clone(),
                        metadata,
                    })
            })
            .collect();

        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));
        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let count = std::fs::read_dir(location)?
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .count();
        Ok(count)
    }
}
