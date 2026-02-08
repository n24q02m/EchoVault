//! OpenAI Codex CLI Extractor
//!
//! Extracts conversation history from OpenAI Codex CLI.
//! Sessions are stored as JSONL files in:
//! - ~/.codex/sessions/YYYY/MM/DD/rollout-<timestamp>.jsonl (per-session)
//! - ~/.codex/history.jsonl (aggregate transcript)
//!
//! Config: ~/.codex/config.toml or .codex/config.toml (project-level)

use super::{Extractor, SessionFile, SessionMetadata};
use crate::utils::wsl;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde_json::Value;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// OpenAI Codex CLI Extractor
pub struct CodexExtractor {
    /// Paths to ~/.codex/ directories
    codex_dirs: Vec<PathBuf>,
}

impl CodexExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut codex_dirs = Vec::new();

        // Check CODEX_HOME env var first
        if let Ok(codex_home) = std::env::var("CODEX_HOME") {
            codex_dirs.push(PathBuf::from(codex_home));
        }

        // Prefer $HOME env variable
        if let Ok(home) = std::env::var("HOME") {
            let path = PathBuf::from(home).join(".codex");
            if !codex_dirs.contains(&path) {
                codex_dirs.push(path);
            }
        }

        // Fallback: dirs crate
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".codex");
            if !codex_dirs.contains(&path) {
                codex_dirs.push(path);
            }
        }

        // Windows: Scan WSL for Codex CLI data
        for wsl_path in wsl::find_wsl_paths(".codex") {
            if !codex_dirs.contains(&wsl_path) {
                codex_dirs.push(wsl_path);
            }
        }

        Self { codex_dirs }
    }

    /// Find all JSONL session files recursively in sessions/ directory.
    /// Structure: sessions/YYYY/MM/DD/rollout-<timestamp>.jsonl
    fn find_session_files(sessions_dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();

        if !sessions_dir.exists() {
            return files;
        }

        // Walk up to 4 levels: sessions/YYYY/MM/DD/*.jsonl
        Self::walk_dir(sessions_dir, 0, 4, &mut files);
        files
    }

    fn walk_dir(dir: &Path, depth: usize, max_depth: usize, files: &mut Vec<PathBuf>) {
        if depth > max_depth {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    Self::walk_dir(&path, depth + 1, max_depth, files);
                } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                    files.push(path);
                }
            }
        }
    }

    /// Extract metadata from a Codex session JSONL file.
    fn extract_session_metadata(&self, path: &Path) -> Option<SessionMetadata> {
        let file = std::fs::File::open(path).ok()?;
        let reader = std::io::BufReader::new(file);

        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        if session_id.is_empty() {
            return None;
        }

        let mut title: Option<String> = None;
        let mut created_at: Option<DateTime<Utc>> = None;

        // Parse first lines for metadata
        for line in reader.lines().take(30).flatten() {
            if let Ok(obj) = serde_json::from_str::<Value>(&line) {
                // Get timestamp
                if created_at.is_none() {
                    created_at = obj
                        .get("timestamp")
                        .or_else(|| obj.get("created_at"))
                        .and_then(|v| {
                            v.as_str()
                                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                                .map(|dt| dt.with_timezone(&Utc))
                                .or_else(|| {
                                    v.as_f64().and_then(|ts| {
                                        DateTime::<Utc>::from_timestamp(ts as i64, 0)
                                    })
                                })
                        });
                }

                // Look for first user message as title
                if title.is_none() {
                    let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or_default();
                    let msg_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or_default();

                    if role == "user" || msg_type == "user" || msg_type == "input" {
                        let text = obj
                            .get("content")
                            .or_else(|| obj.get("text"))
                            .or_else(|| obj.get("message"))
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        if let Some(text) = text {
                            if text.len() > 5 {
                                let truncated: String = text.chars().take(60).collect();
                                title = Some(if text.chars().count() > 60 {
                                    format!("{}...", truncated)
                                } else {
                                    truncated
                                });
                            }
                        }
                    }
                }

                if title.is_some() && created_at.is_some() {
                    break;
                }
            }
        }

        // Fallback timestamp from file metadata or filename
        if created_at.is_none() {
            // Try to parse timestamp from filename: rollout-<timestamp>.jsonl
            created_at = session_id
                .strip_prefix("rollout-")
                .and_then(|ts_str| ts_str.parse::<i64>().ok())
                .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
                .or_else(|| {
                    std::fs::metadata(path)
                        .ok()
                        .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0))
                });
        }

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if file_size < 10 {
            return None;
        }

        Some(SessionMetadata {
            id: session_id,
            source: "codex".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: path.to_path_buf(),
            file_size,
            workspace_name: Some("Codex CLI".to_string()),
            ide_origin: None,
        })
    }
}

impl Default for CodexExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for CodexExtractor {
    fn source_name(&self) -> &'static str {
        "codex"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let mut locations = Vec::new();

        for codex_dir in &self.codex_dirs {
            let sessions_dir = codex_dir.join("sessions");
            if sessions_dir.exists() && sessions_dir.is_dir() {
                locations.push(sessions_dir);
            }
        }

        Ok(locations)
    }

    fn get_workspace_name(&self, _location: &Path) -> String {
        "Codex CLI".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let jsonl_files = Self::find_session_files(location);

        let mut sessions: Vec<SessionFile> = jsonl_files
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
        Ok(Self::find_session_files(location).len())
    }
}
