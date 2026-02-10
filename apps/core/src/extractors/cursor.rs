//! Cursor AI Editor Extractor
//!
//! Extracts chat history from Cursor AI Editor.
//! ONLY COPY the raw state.vscdb file, DO NOT parse/transform content.
//!
//! Cursor stores ALL chat data in a single SQLite database:
//! - Windows: %APPDATA%\Cursor\User\globalStorage\state.vscdb
//! - macOS: ~/Library/Application Support/Cursor/User/globalStorage/state.vscdb
//! - Linux: ~/.config/Cursor/User/globalStorage/state.vscdb
//!
//! The database has a `cursorDiskKV` table with key-value pairs:
//! - `composerData:<uuid>` — JSON blob with conversation metadata (name, createdAt, status, etc.)
//! - `agentKv:blob:<hash>` — JSON blob with individual messages (OpenAI-compatible format)

use super::{Extractor, SessionFile, SessionMetadata};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Cursor AI Editor Extractor
pub struct CursorExtractor {
    /// Paths to state.vscdb files
    db_paths: Vec<PathBuf>,
}

impl CursorExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        let mut db_paths = Vec::new();

        // Prefer reading from HOME env variable (for testing with HOME override)
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(home);
            for variant in &["Cursor", "Cursor - Insiders"] {
                let db = home_path
                    .join(".config")
                    .join(variant)
                    .join("User/globalStorage/state.vscdb");
                if !db_paths.contains(&db) {
                    db_paths.push(db);
                }
            }
        }

        // Fallback: Get path per platform via dirs crate
        if let Some(config_dir) = dirs::config_dir() {
            for variant in &["Cursor", "Cursor - Insiders"] {
                let db = config_dir
                    .join(variant)
                    .join("User/globalStorage/state.vscdb");
                if !db_paths.contains(&db) {
                    db_paths.push(db);
                }
            }
        }

        // NOTE: On Windows, dirs::config_dir() returns %APPDATA% (Roaming)
        // which is the correct location for Cursor storage.

        Self { db_paths }
    }

    /// Extract composer metadata from a composerData JSON blob.
    fn extract_composer_metadata(
        key: &str,
        json_str: &str,
        db_path: &Path,
        db_size: u64,
    ) -> Option<SessionMetadata> {
        let json: Value = serde_json::from_str(json_str).ok()?;

        let composer_id = json
            .get("composerId")
            .and_then(|v| v.as_str())
            .or_else(|| {
                // Fallback: extract UUID from key "composerData:<uuid>"
                key.strip_prefix("composerData:")
            })?
            .to_string();

        let title = json
            .get("name")
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

        let created_at = json
            .get("createdAt")
            .and_then(|v| v.as_i64())
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        Some(SessionMetadata {
            id: composer_id,
            source: "cursor".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(),
            original_path: db_path.to_path_buf(),
            file_size: db_size,
            workspace_name: Some("Cursor".to_string()),
            ide_origin: None,
        })
    }

    /// List all composers from a state.vscdb database.
    fn list_composers_from_db(db_path: &Path) -> Vec<SessionFile> {
        let db = match rusqlite::Connection::open_with_flags(
            db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(db) => db,
            Err(_) => return Vec::new(),
        };

        let db_size = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

        // Query all composerData entries from cursorDiskKV
        let mut stmt = match db
            .prepare("SELECT key, value FROM cursorDiskKV WHERE key LIKE 'composerData:%'")
        {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };

        stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            Ok((key, value))
        })
        .ok()
        .map(|iter| {
            iter.filter_map(|r| r.ok())
                .filter_map(|(key, value)| {
                    Self::extract_composer_metadata(&key, &value, db_path, db_size).map(
                        |metadata| SessionFile {
                            source_path: db_path.to_path_buf(),
                            metadata,
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default()
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
        let mut locations = Vec::new();

        for db_path in &self.db_paths {
            if db_path.exists() {
                // Use the parent directory (globalStorage/) as the "location"
                if let Some(parent) = db_path.parent() {
                    if !locations.contains(&parent.to_path_buf()) {
                        locations.push(parent.to_path_buf());
                    }
                }
            }
        }

        Ok(locations)
    }

    fn get_workspace_name(&self, _location: &Path) -> String {
        "Cursor".to_string()
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        let db_path = location.join("state.vscdb");
        if !db_path.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Self::list_composers_from_db(&db_path);

        // Sort by creation time (newest first)
        sessions.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

        Ok(sessions)
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        let db_path = location.join("state.vscdb");
        if !db_path.exists() {
            return Ok(0);
        }

        let db = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        let count: usize = db.query_row(
            "SELECT COUNT(*) FROM cursorDiskKV WHERE key LIKE 'composerData:%'",
            [],
            |row| row.get(0),
        )?;

        Ok(count)
    }

    /// Custom copy: copy the entire state.vscdb file (similar to Zed's threads.db approach).
    fn copy_to_vault(&self, session: &SessionFile, vault_dir: &Path) -> Result<Option<PathBuf>> {
        let source_dir = vault_dir.join(self.source_name());
        std::fs::create_dir_all(&source_dir)?;

        // Determine which variant this is from (Cursor vs Cursor - Insiders)
        let variant_name = session
            .source_path
            .ancestors()
            .find_map(|p| {
                let name = p.file_name()?.to_str()?;
                if name == "Cursor" || name == "Cursor - Insiders" {
                    Some(name.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "Cursor".to_string());

        let dest_filename = format!("{}.vscdb", variant_name.to_lowercase().replace(' ', "-"));
        let dest_path = source_dir.join(&dest_filename);

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
