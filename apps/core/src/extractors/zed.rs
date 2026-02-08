//! Zed Editor AI Assistant Extractor
//!
//! Extracts AI conversation history from the Zed editor.
//! Zed has TWO storage systems:
//!
//! ## 1. Agent Threads (Primary - SQLite + zstd)
//! - Path: `{data_dir}/threads/threads.db`
//! - Format: SQLite with zstd-compressed JSON blobs
//! - Schema: `threads(id TEXT PK, summary TEXT, updated_at TEXT, data_type TEXT, data BLOB)`
//! - `data_type` = "json" (raw) or "zstd" (compressed)
//!
//! ## 2. Text Threads (Legacy - JSON files)
//! - Path: `{state_dir}/conversations/*.zed.json` or `{config_dir}/conversations/*.zed.json`
//! - Format: JSON with text buffer + message boundaries
//!
//! Platform paths:
//! - Linux: `~/.local/share/zed/` (Agent), `~/.local/state/zed/conversations/` (Text)
//! - macOS: `~/.local/share/Zed/` (Agent), `~/Library/Application Support/Zed/conversations/` (Text)
//! - Windows: `%LocalAppData%\Zed\` (both)

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Zed Editor AI Assistant Extractor
pub struct ZedExtractor {
    /// Paths to Agent Threads SQLite DBs (threads.db files)
    agent_thread_dbs: Vec<PathBuf>,
    /// Paths to directories containing legacy Text Thread JSON files
    text_thread_dirs: Vec<PathBuf>,
}

impl ZedExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut agent_thread_dbs = Vec::new();
        let mut text_thread_dirs = Vec::new();

        #[cfg(target_os = "linux")]
        {
            // Agent Threads: ~/.local/share/zed/threads/threads.db
            if let Some(data_dir) = dirs::data_dir() {
                let db = data_dir.join("zed").join("threads").join("threads.db");
                if db.exists() {
                    agent_thread_dbs.push(db);
                }
            }
            // Text Threads: $XDG_STATE_HOME/zed/conversations/ or ~/.local/state/zed/conversations/
            if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
                let dir = PathBuf::from(state_home).join("zed").join("conversations");
                if dir.exists() {
                    text_thread_dirs.push(dir);
                }
            } else if let Some(home) = dirs::home_dir() {
                let dir = home
                    .join(".local")
                    .join("state")
                    .join("zed")
                    .join("conversations");
                if dir.exists() {
                    text_thread_dirs.push(dir);
                }
            }
            // Fallback: $XDG_DATA_HOME/zed/conversations/
            if let Some(data_dir) = dirs::data_dir() {
                let dir = data_dir.join("zed").join("conversations");
                if dir.exists() && !text_thread_dirs.contains(&dir) {
                    text_thread_dirs.push(dir);
                }
            }
            // Config fallback
            if let Some(config_dir) = dirs::config_dir() {
                let dir = config_dir.join("zed").join("conversations");
                if dir.exists() && !text_thread_dirs.contains(&dir) {
                    text_thread_dirs.push(dir);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // Agent Threads: ~/.local/share/Zed/threads/threads.db
            if let Some(data_dir) = dirs::data_dir() {
                let db = data_dir.join("Zed").join("threads").join("threads.db");
                if db.exists() {
                    agent_thread_dbs.push(db);
                }
            }
            // Text Threads: ~/Library/Application Support/Zed/conversations/
            if let Some(data_dir) = dirs::data_dir() {
                let dir = data_dir.join("Zed").join("conversations");
                if dir.exists() {
                    text_thread_dirs.push(dir);
                }
            }
            // Fallback: ~/.config/zed/conversations/
            if let Some(config_dir) = dirs::config_dir() {
                let dir = config_dir.join("zed").join("conversations");
                if dir.exists() && !text_thread_dirs.contains(&dir) {
                    text_thread_dirs.push(dir);
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Agent Threads: %LocalAppData%\Zed\threads\threads.db
            if let Some(local_appdata) = dirs::data_local_dir() {
                let db = local_appdata.join("Zed").join("threads").join("threads.db");
                if db.exists() {
                    agent_thread_dbs.push(db);
                }
            }
            // Text Threads: %LocalAppData%\Zed\conversations\
            if let Some(local_appdata) = dirs::data_local_dir() {
                let dir = local_appdata.join("Zed").join("conversations");
                if dir.exists() {
                    text_thread_dirs.push(dir);
                }
            }
            // Also check %APPDATA%
            if let Some(appdata) = dirs::config_dir() {
                let dir = appdata.join("Zed").join("conversations");
                if dir.exists() && !text_thread_dirs.contains(&dir) {
                    text_thread_dirs.push(dir);
                }
            }
        }

        // ZED_DATA_DIR env var override
        if let Ok(dir) = std::env::var("ZED_DATA_DIR") {
            let base = PathBuf::from(&dir);
            let db = base.join("threads").join("threads.db");
            if db.exists() && !agent_thread_dbs.contains(&db) {
                agent_thread_dbs.push(db);
            }
            let conv_dir = base.join("conversations");
            if conv_dir.exists() && !text_thread_dirs.contains(&conv_dir) {
                text_thread_dirs.push(conv_dir);
            }
        }

        Self {
            agent_thread_dbs,
            text_thread_dirs,
        }
    }

    /// List sessions from Agent Threads SQLite database.
    fn list_agent_thread_sessions(&self, db_path: &Path) -> Vec<SessionFile> {
        let db = match rusqlite::Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(db) => db,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match db.prepare(
            "SELECT id, summary, updated_at, data_type, length(data) \
             FROM threads ORDER BY updated_at DESC",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };

        stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let summary: String = row.get(1)?;
            let updated_at: String = row.get(2)?;
            let _data_type: String = row.get(3)?;
            let data_len: i64 = row.get(4)?;

            let created_at = DateTime::parse_from_rfc3339(&updated_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc));

            let title = if summary.is_empty() {
                None
            } else {
                let truncated: String = summary.chars().take(80).collect();
                if summary.chars().count() > 80 {
                    Some(format!("{}...", truncated))
                } else {
                    Some(truncated)
                }
            };

            Ok(SessionFile {
                source_path: db_path.to_path_buf(),
                metadata: SessionMetadata {
                    id: format!("zed-agent-{}", id),
                    source: "zed".to_string(),
                    title,
                    created_at,
                    vault_path: PathBuf::new(),
                    original_path: db_path.to_path_buf(),
                    file_size: data_len as u64,
                    workspace_name: Some("Zed Agent".to_string()),
                    ide_origin: None,
                },
            })
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Extract metadata from a legacy .zed.json conversation file.
    fn extract_text_thread_metadata(&self, path: &Path) -> Option<SessionMetadata> {
        let content = std::fs::read_to_string(path).ok()?;
        let conversation: Value = serde_json::from_str(&content).ok()?;

        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("zed-text-{}", s))?;

        // Skip empty conversations: check for either messages array or text field
        let has_messages = conversation
            .get("messages")
            .and_then(|v| v.as_array())
            .is_some_and(|arr| !arr.is_empty());
        let has_text = conversation
            .get("text")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        if !has_messages && !has_text {
            return None;
        }

        let title = conversation
            .get("summary")
            .or_else(|| conversation.get("title"))
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

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        let created_at = conversation
            .get("updated_at")
            .or_else(|| conversation.get("created_at"))
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .or_else(|| {
                        v.as_f64()
                            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0))
                    })
            })
            .or_else(|| {
                std::fs::metadata(path)
                    .ok()
                    .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0))
            });

        Some(SessionMetadata {
            id: session_id,
            source: "zed".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: path.to_path_buf(),
            file_size,
            workspace_name: Some("Zed".to_string()),
            ide_origin: None,
        })
    }
}

impl Default for ZedExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for ZedExtractor {
    fn source_name(&self) -> &'static str {
        "zed"
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        let mut locations = Vec::new();

        // Agent threads databases (use parent dir as "location")
        for db_path in &self.agent_thread_dbs {
            if let Some(parent) = db_path.parent() {
                locations.push(parent.to_path_buf());
            }
        }

        // Text thread directories
        for dir in &self.text_thread_dirs {
            if !locations.contains(dir) {
                locations.push(dir.clone());
            }
        }

        Ok(locations)
    }

    fn get_workspace_name(&self, _location: &Path) -> String {
        "Zed".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let mut sessions = Vec::new();

        // Check if this location has an Agent Threads DB
        let threads_db = location.join("threads.db");
        if threads_db.exists() {
            sessions.extend(self.list_agent_thread_sessions(&threads_db));
        }

        // Check if this location has Text Thread JSON files
        let mut json_files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(location) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();
                    if name.ends_with(".zed.json") || name.ends_with(".json") {
                        json_files.push(path);
                    }
                }
            }
        }

        if !json_files.is_empty() {
            let text_sessions: Vec<SessionFile> = json_files
                .par_iter()
                .filter_map(|path| {
                    self.extract_text_thread_metadata(path)
                        .map(|metadata| SessionFile {
                            source_path: path.clone(),
                            metadata,
                        })
                })
                .collect();
            sessions.extend(text_sessions);
        }

        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));
        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let mut count = 0;

        let threads_db = location.join("threads.db");
        if threads_db.exists() {
            if let Ok(db) = rusqlite::Connection::open_with_flags(
                &threads_db,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            ) {
                if let Ok(c) = db.query_row("SELECT COUNT(*) FROM threads", [], |row| {
                    row.get::<_, usize>(0)
                }) {
                    count += c;
                }
            }
        }

        if let Ok(entries) = std::fs::read_dir(location) {
            count += entries
                .flatten()
                .filter(|e| {
                    let name = e
                        .path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                        .to_string();
                    name.ends_with(".zed.json") || name.ends_with(".json")
                })
                .count();
        }

        Ok(count)
    }

    /// Custom copy: for Agent Threads, copy the entire threads.db.
    /// For Text Threads, use standard file copy.
    fn copy_to_vault(&self, session: &SessionFile, vault_dir: &Path) -> Result<Option<PathBuf>> {
        let source_dir = vault_dir.join(self.source_name());
        std::fs::create_dir_all(&source_dir)?;

        if session.metadata.id.starts_with("zed-agent-") {
            // Agent threads: copy the threads.db once
            let dest_path = source_dir.join("threads.db");
            let should_copy = if dest_path.exists() {
                let src_meta = session.source_path.metadata()?;
                let dest_meta = dest_path.metadata()?;
                src_meta.modified()? > dest_meta.modified()? || src_meta.len() != dest_meta.len()
            } else {
                true
            };
            if should_copy {
                std::fs::copy(&session.source_path, &dest_path)?;
                Ok(Some(dest_path))
            } else {
                Ok(None)
            }
        } else {
            // Text threads: standard file copy
            let filename = session
                .source_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let dest_path = source_dir.join(&filename);
            let should_copy = if dest_path.exists() {
                let src_meta = session.source_path.metadata()?;
                let dest_meta = dest_path.metadata()?;
                src_meta.modified()? > dest_meta.modified()? || src_meta.len() != dest_meta.len()
            } else {
                true
            };
            if should_copy {
                std::fs::copy(&session.source_path, &dest_path)?;
                Ok(Some(dest_path))
            } else {
                Ok(None)
            }
        }
    }
}
