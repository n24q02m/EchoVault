//! OpenCode Terminal AI Extractor
//!
//! Extracts chat session history from OpenCode (github.com/opencode-ai/opencode).
//! OpenCode v1.x stores sessions as 3-tier JSON files using XDG paths:
//!
//! Storage: ~/.local/share/opencode/storage/
//!   - session/<id>.json   (session metadata)
//!   - message/<id>.json   (message metadata: role, model, tokens)
//!   - part/<id>.json      (actual content: text, tool calls)
//!   - project/<id>.json   (project metadata)
//!   - session_diff/<id>.json (diff data)
//!
//! The extractor copies all JSON files from the storage directory to the vault.

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// OpenCode Extractor
pub struct OpenCodeExtractor {
    /// Paths to opencode storage directories.
    storage_dirs: Vec<PathBuf>,
}

impl OpenCodeExtractor {
    /// Create new extractor.
    pub fn new() -> Self {
        let mut storage_dirs = Vec::new();

        // Check OPENCODE_HOME env var
        if let Ok(home) = std::env::var("OPENCODE_HOME") {
            storage_dirs.push(PathBuf::from(home).join("storage"));
        }

        // XDG data directory: ~/.local/share/opencode/storage/
        if let Some(data_dir) = dirs::data_dir() {
            let path = data_dir.join("opencode").join("storage");
            if !storage_dirs.contains(&path) {
                storage_dirs.push(path);
            }
        }

        // Fallback: $HOME/.local/share/opencode/storage/
        if let Ok(home) = std::env::var("HOME") {
            let path = PathBuf::from(home).join(".local/share/opencode/storage");
            if !storage_dirs.contains(&path) {
                storage_dirs.push(path);
            }
        }

        // Additional fallback via dirs::home_dir
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".local/share/opencode/storage");
            if !storage_dirs.contains(&path) {
                storage_dirs.push(path);
            }
        }

        Self { storage_dirs }
    }

    /// Find session JSON files in the session/ subdirectory.
    fn find_session_files(storage_dir: &Path) -> Vec<PathBuf> {
        let session_dir = storage_dir.join("session");
        if !session_dir.exists() || !session_dir.is_dir() {
            return Vec::new();
        }

        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&session_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                    files.push(path);
                }
            }
        }
        files
    }

    /// Extract session metadata from a session JSON file.
    fn extract_session_metadata(
        session_path: &Path,
        storage_dir: &Path,
    ) -> Option<SessionMetadata> {
        let content = std::fs::read_to_string(session_path).ok()?;
        let json: Value = serde_json::from_str(&content).ok()?;

        let session_id = json
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())?;

        let title = json
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                json.get("slug")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        let created_at = json
            .get("time")
            .and_then(|t| t.get("created"))
            .and_then(|v| v.as_i64())
            .and_then(|ms| {
                DateTime::<Utc>::from_timestamp(ms / 1000, ((ms % 1000) * 1_000_000) as u32)
            });

        let workspace_name = json.get("directory").and_then(|v| v.as_str()).map(|dir| {
            dir.replace('\\', "/")
                .rsplit('/')
                .next()
                .unwrap_or(dir)
                .to_string()
        });

        // Calculate total size: session file + associated messages + parts
        let file_size = std::fs::metadata(session_path)
            .map(|m| m.len())
            .unwrap_or(0);

        Some(SessionMetadata {
            id: session_id,
            source: "opencode".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: storage_dir.to_path_buf(),
            file_size,
            workspace_name,
            ide_origin: None,
        })
    }
}

impl Default for OpenCodeExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for OpenCodeExtractor {
    fn source_name(&self) -> &'static str {
        "opencode"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let mut locations = Vec::new();

        for storage_dir in &self.storage_dirs {
            if storage_dir.exists() && storage_dir.is_dir() {
                // Check if there are session files
                let session_dir = storage_dir.join("session");
                if session_dir.exists() && session_dir.is_dir() {
                    locations.push(storage_dir.clone());
                }
            }
        }

        Ok(locations)
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        // The location is the storage/ directory.
        // Try to read the first session's directory field.
        let session_dir = location.join("session");
        if let Ok(entries) = std::fs::read_dir(&session_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(json) = serde_json::from_str::<Value>(&content) {
                            if let Some(dir) = json.get("directory").and_then(|v| v.as_str()) {
                                return dir
                                    .replace('\\', "/")
                                    .rsplit('/')
                                    .next()
                                    .unwrap_or(dir)
                                    .to_string();
                            }
                        }
                    }
                }
            }
        }
        "OpenCode".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let session_files = Self::find_session_files(location);
        let mut sessions: Vec<SessionFile> = session_files
            .iter()
            .filter_map(|path| {
                Self::extract_session_metadata(path, location).map(|metadata| SessionFile {
                    source_path: location.to_path_buf(),
                    metadata,
                })
            })
            .collect();

        // Sort by creation time (newest first)
        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));
        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        Ok(Self::find_session_files(location).len())
    }

    /// Custom copy: copy the entire storage directory structure to vault.
    fn copy_to_vault(&self, session: &SessionFile, vault_dir: &Path) -> Result<Option<PathBuf>> {
        let source_dir = vault_dir.join(self.source_name());
        std::fs::create_dir_all(&source_dir)?;

        // Use session ID as subdirectory
        let session_id = &session.metadata.id;
        let dest_session_dir = source_dir.join(session_id);
        std::fs::create_dir_all(&dest_session_dir)?;

        let storage_dir = &session.source_path;
        let mut copied = false;

        // Copy relevant files for this session from all subdirectories
        let subdirs = ["session", "message", "part", "project", "session_diff"];
        for subdir in &subdirs {
            let src_subdir = storage_dir.join(subdir);
            if !src_subdir.exists() {
                continue;
            }

            let dest_subdir = dest_session_dir.join(subdir);
            std::fs::create_dir_all(&dest_subdir)?;

            if let Ok(entries) = std::fs::read_dir(&src_subdir) {
                for entry in entries.flatten() {
                    let src_path = entry.path();
                    if src_path.is_file() && src_path.extension().is_some_and(|ext| ext == "json") {
                        // For session/: only copy the matching session file
                        // For message/part/: filter by sessionID in content
                        let should_copy = if *subdir == "session" {
                            src_path
                                .file_stem()
                                .and_then(|n| n.to_str())
                                .is_some_and(|name| name == session_id)
                        } else {
                            // Copy all files — the parser will filter by sessionID
                            true
                        };

                        if should_copy {
                            let filename = src_path.file_name().unwrap_or_default();
                            let dest_path = dest_subdir.join(filename);

                            let do_copy = if dest_path.exists() {
                                let src_meta = src_path.metadata()?;
                                let dest_meta = dest_path.metadata()?;
                                src_meta.modified()? > dest_meta.modified()?
                                    || src_meta.len() != dest_meta.len()
                            } else {
                                true
                            };

                            if do_copy {
                                std::fs::copy(&src_path, &dest_path)?;
                                copied = true;
                            }
                        }
                    }
                }
            }
        }

        if copied {
            Ok(Some(dest_session_dir))
        } else {
            Ok(None)
        }
    }
}
