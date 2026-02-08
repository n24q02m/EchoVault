//! Continue.dev VS Code/JetBrains Extension Extractor
//!
//! Extracts chat session history from Continue.dev.
//! Sessions are stored as individual JSON files:
//! - ~/.continue/sessions/{sessionId}.json (per-session)
//! - ~/.continue/sessions/sessions.json (index file, optional)
//!
//! Session JSON structure (TypeScript):
//! ```ts
//! interface Session {
//!   sessionId: string;
//!   title: string;
//!   workspaceDirectory: string;
//!   history: ChatHistoryItem[];
//!   mode?: string;    // "chat" | "agent" | "plan" | "background"
//!   chatModelTitle?: string;
//! }
//! interface ChatHistoryItem {
//!   message: ChatMessage;
//!   contextItems: ContextItemWithId[];
//! }
//! type ChatMessage = UserChatMessage | AssistantChatMessage | SystemChatMessage
//!   | ThinkingChatMessage | ToolResultChatMessage;
//! ```
//!
//! Paths:
//! - Linux/macOS: ~/.continue/sessions/
//! - Windows: %USERPROFILE%\.continue\sessions\

use super::{Extractor, ExtractorKind, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Continue.dev Extractor
pub struct ContinueDevExtractor {
    /// Paths to ~/.continue/ directories
    continue_dirs: Vec<PathBuf>,
}

impl ContinueDevExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut continue_dirs = Vec::new();

        // Check CONTINUE_GLOBAL_DIR env var first
        if let Ok(dir) = std::env::var("CONTINUE_GLOBAL_DIR") {
            continue_dirs.push(PathBuf::from(dir));
        }

        // Standard home directory path
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".continue");
            if !continue_dirs.contains(&path) {
                continue_dirs.push(path);
            }
        }

        Self { continue_dirs }
    }

    /// Extract metadata from a Continue session JSON file.
    fn extract_session_metadata(&self, path: &Path) -> Option<SessionMetadata> {
        let content = std::fs::read_to_string(path).ok()?;
        let session: Value = serde_json::from_str(&content).ok()?;

        let session_id = session
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;

        let title = session
            .get("title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| {
                let truncated: String = s.chars().take(80).collect();
                if s.chars().count() > 80 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            });

        let workspace = session
            .get("workspaceDirectory")
            .and_then(|v| v.as_str())
            .and_then(|ws| {
                // Extract just the directory name from the full path
                PathBuf::from(ws)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            });

        // Check if there are actual messages
        let history = session.get("history").and_then(|v| v.as_array())?;
        if history.is_empty() {
            return None;
        }

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        // Timestamp from file metadata
        let created_at = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.created().or_else(|_| m.modified()).ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));

        Some(SessionMetadata {
            id: session_id,
            source: "continue-dev".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: path.to_path_buf(),
            file_size,
            workspace_name: workspace,
            ide_origin: None,
        })
    }
}

impl Default for ContinueDevExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for ContinueDevExtractor {
    fn source_name(&self) -> &'static str {
        "continue-dev"
    }

    fn extractor_kind(&self) -> ExtractorKind {
        ExtractorKind::Extension
    }

    fn supported_ides(&self) -> &'static [&'static str] {
        &["VS Code", "JetBrains"]
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let mut locations = Vec::new();

        for continue_dir in &self.continue_dirs {
            let sessions_dir = continue_dir.join("sessions");
            if sessions_dir.exists() && sessions_dir.is_dir() {
                locations.push(sessions_dir);
            }
        }

        Ok(locations)
    }

    fn get_workspace_name(&self, _location: &Path) -> String {
        "Continue.dev".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let mut json_files = Vec::new();

        if let Ok(entries) = std::fs::read_dir(location) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path.extension().is_some_and(|ext| ext == "json")
                    && path.file_name().is_some_and(|name| name != "sessions.json")
                {
                    json_files.push(path);
                }
            }
        }

        let mut sessions: Vec<SessionFile> = json_files
            .par_iter()
            .filter_map(|path| {
                self.extract_session_metadata(path)
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
            .filter(|e| {
                let path = e.path();
                path.is_file()
                    && path.extension().is_some_and(|ext| ext == "json")
                    && path.file_name().is_some_and(|name| name != "sessions.json")
            })
            .count();
        Ok(count)
    }
}
