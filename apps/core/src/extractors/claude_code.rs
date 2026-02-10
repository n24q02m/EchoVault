//! Claude Code CLI Extractor
//!
//! Extracts conversation history from Claude Code (Anthropic CLI agent).
//! Conversations are stored as JSONL files in ~/.claude/projects/<path-encoded-dir>/*.jsonl
//! History index is at ~/.claude/history.jsonl
//!
//! Path encoding: /Users/bill/My Project -> -Users-bill-My-Project
//! (special chars /, spaces, ~ are replaced with -)

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde_json::Value;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Claude Code CLI Extractor
pub struct ClaudeCodeExtractor {
    /// Paths to ~/.claude/ directories
    claude_dirs: Vec<PathBuf>,
}

impl ClaudeCodeExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut claude_dirs = Vec::new();

        // Prefer $HOME env variable
        if let Ok(home) = std::env::var("HOME") {
            claude_dirs.push(PathBuf::from(home).join(".claude"));
        }

        // Fallback: dirs crate
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".claude");
            if !claude_dirs.contains(&path) {
                claude_dirs.push(path);
            }
        }

        Self { claude_dirs }
    }

    /// Find all project directories under ~/.claude/projects/
    fn find_project_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        for claude_dir in &self.claude_dirs {
            let projects_dir = claude_dir.join("projects");
            if !projects_dir.exists() || !projects_dir.is_dir() {
                continue;
            }

            // Each subdirectory is a project (path-encoded)
            if let Ok(entries) = std::fs::read_dir(&projects_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        dirs.push(path);
                    }
                }
            }
        }

        dirs
    }

    /// Decode a Claude Code path-encoded directory name back to a readable project name.
    /// e.g., "-Users-bill-My-Project" -> "My-Project" (just the last segment)
    fn decode_project_name(encoded: &str) -> String {
        // The encoded path has leading dash and dashes for separators
        // Extract the last meaningful segment
        let parts: Vec<&str> = encoded.split('-').filter(|s| !s.is_empty()).collect();
        parts.last().unwrap_or(&encoded).to_string()
    }

    /// Extract metadata from a Claude Code JSONL conversation file.
    fn extract_session_metadata(&self, path: &Path, project_name: &str) -> Option<SessionMetadata> {
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
        let mut line_count = 0;

        // Read first few lines to extract metadata
        for line in reader.lines().take(50).flatten() {
            line_count += 1;
            if let Ok(obj) = serde_json::from_str::<Value>(&line) {
                // Get timestamp from first entry
                if created_at.is_none() {
                    created_at = obj
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .or_else(|| {
                            obj.get("createdAt")
                                .and_then(|v| v.as_str())
                                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                                .map(|dt| dt.with_timezone(&Utc))
                        });
                }

                // Look for first human/user message as title
                if title.is_none() {
                    let role = obj.get("role").and_then(|v| v.as_str()).unwrap_or_default();

                    if role == "human" || role == "user" {
                        let text = obj.get("content").and_then(|c| {
                            c.as_str().map(|s| s.to_string()).or_else(|| {
                                c.as_array().and_then(|arr| {
                                    arr.iter().find_map(|item| {
                                        if item.get("type").and_then(|t| t.as_str()) == Some("text")
                                        {
                                            item.get("text")
                                                .and_then(|t| t.as_str())
                                                .map(String::from)
                                        } else {
                                            None
                                        }
                                    })
                                })
                            })
                        });

                        if let Some(text) = text {
                            let truncated: String = text.chars().take(60).collect();
                            title = Some(if text.chars().count() > 60 {
                                format!("{}...", truncated)
                            } else {
                                truncated
                            });
                        }
                    }
                }

                if title.is_some() && created_at.is_some() {
                    break;
                }
            }
        }

        // Fallback timestamp from file metadata
        if created_at.is_none() {
            created_at = std::fs::metadata(path)
                .ok()
                .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));
        }

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        // Skip empty or very small files
        if file_size < 10 || line_count == 0 {
            return None;
        }

        Some(SessionMetadata {
            id: session_id,
            source: "claude-code".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: path.to_path_buf(),
            file_size,
            workspace_name: Some(project_name.to_string()),
            ide_origin: None,
        })
    }
}

impl Default for ClaudeCodeExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for ClaudeCodeExtractor {
    fn source_name(&self) -> &'static str {
        "claude-code"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let project_dirs = self.find_project_dirs();
        // Only return dirs that have .jsonl files
        Ok(project_dirs
            .into_iter()
            .filter(|dir| {
                std::fs::read_dir(dir)
                    .map(|entries| {
                        entries
                            .flatten()
                            .any(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
                    })
                    .unwrap_or(false)
            })
            .collect())
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        // location is ~/.claude/projects/<path-encoded-dir>/
        location
            .file_name()
            .and_then(|n| n.to_str())
            .map(Self::decode_project_name)
            .unwrap_or_else(|| "Claude Code".to_string())
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let project_name = self.get_workspace_name(location);

        let jsonl_paths: Vec<PathBuf> = std::fs::read_dir(location)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "jsonl"))
            .collect();

        let mut sessions: Vec<SessionFile> = jsonl_paths
            .par_iter()
            .filter_map(|path| {
                self.extract_session_metadata(path, &project_name)
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
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .count();
        Ok(count)
    }
}
