//! SQLite index for metadata - Fast search and filtering of sessions.
//!
//! Index contains basic metadata of each session, allowing:
//! - Full-text search in titles
//! - Filtering by source, workspace, date range
//! - Pagination of results
//!
//! Raw JSON files are still stored separately, index only contains metadata.

use crate::extractors::SessionMetadata;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

/// SQLite index for managing session metadata
pub struct SessionIndex {
    conn: Connection,
}

#[allow(dead_code)]
impl SessionIndex {
    /// Open or create index database
    pub fn open(vault_dir: &Path) -> Result<Self> {
        let db_path = vault_dir.join("index.db");

        // Create directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Cannot open index database: {}", db_path.display()))?;

        let index = Self { conn };
        index.init_schema()?;

        Ok(index)
    }

    /// Open index database in memory (for testing)
    #[allow(dead_code)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let index = Self { conn };
        index.init_schema()?;
        Ok(index)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        // Main table for metadata
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                title TEXT,
                created_at TEXT,
                vault_path TEXT NOT NULL,
                original_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                workspace_name TEXT,
                indexed_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        // Index for fast searching
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_source ON sessions(source)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_created_at ON sessions(created_at)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_workspace ON sessions(workspace_name)",
            [],
        )?;

        // Full-text search index cho title
        self.conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
                id,
                title,
                workspace_name,
                content='sessions',
                content_rowid='rowid'
            )",
            [],
        )?;

        // Trigger to automatically update FTS index
        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS sessions_ai AFTER INSERT ON sessions BEGIN
                INSERT INTO sessions_fts(rowid, id, title, workspace_name)
                VALUES (new.rowid, new.id, new.title, new.workspace_name);
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS sessions_ad AFTER DELETE ON sessions BEGIN
                INSERT INTO sessions_fts(sessions_fts, rowid, id, title, workspace_name)
                VALUES ('delete', old.rowid, old.id, old.title, old.workspace_name);
            END",
            [],
        )?;

        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS sessions_au AFTER UPDATE ON sessions BEGIN
                INSERT INTO sessions_fts(sessions_fts, rowid, id, title, workspace_name)
                VALUES ('delete', old.rowid, old.id, old.title, old.workspace_name);
                INSERT INTO sessions_fts(rowid, id, title, workspace_name)
                VALUES (new.rowid, new.id, new.title, new.workspace_name);
            END",
            [],
        )?;

        Ok(())
    }

    /// Add or update a session in the index
    pub fn upsert(&self, metadata: &SessionMetadata) -> Result<()> {
        let created_at = metadata.created_at.map(|dt| dt.to_rfc3339());

        self.conn.execute(
            "INSERT INTO sessions (id, source, title, created_at, vault_path, original_path, file_size, workspace_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                source = excluded.source,
                title = excluded.title,
                created_at = excluded.created_at,
                vault_path = excluded.vault_path,
                original_path = excluded.original_path,
                file_size = excluded.file_size,
                workspace_name = excluded.workspace_name,
                indexed_at = datetime('now')",
            params![
                metadata.id,
                metadata.source,
                metadata.title,
                created_at,
                metadata.vault_path.to_string_lossy().to_string(),
                metadata.original_path.to_string_lossy().to_string(),
                metadata.file_size as i64,
                metadata.workspace_name,
            ],
        )?;

        Ok(())
    }

    /// Add multiple sessions to the index (batch insert)
    pub fn upsert_batch(&mut self, sessions: &[SessionMetadata]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO sessions (id, source, title, created_at, vault_path, original_path, file_size, workspace_name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    source = excluded.source,
                    title = excluded.title,
                    created_at = excluded.created_at,
                    vault_path = excluded.vault_path,
                    original_path = excluded.original_path,
                    file_size = excluded.file_size,
                    workspace_name = excluded.workspace_name,
                    indexed_at = datetime('now')",
            )?;

            for metadata in sessions {
                let created_at = metadata.created_at.map(|dt| dt.to_rfc3339());

                stmt.execute(params![
                    metadata.id,
                    metadata.source,
                    metadata.title,
                    created_at,
                    metadata.vault_path.to_string_lossy().to_string(),
                    metadata.original_path.to_string_lossy().to_string(),
                    metadata.file_size as i64,
                    metadata.workspace_name,
                ])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    /// Get all sessions (with pagination)
    pub fn list(&self, limit: usize, offset: usize) -> Result<Vec<SessionMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source, title, created_at, vault_path, original_path, file_size, workspace_name
             FROM sessions
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            Ok(SessionMetadataRow {
                id: row.get(0)?,
                source: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                vault_path: row.get(4)?,
                original_path: row.get(5)?,
                file_size: row.get(6)?,
                workspace_name: row.get(7)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?.into());
        }

        Ok(sessions)
    }

    /// Full-text search in titles and workspace names
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SessionMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.source, s.title, s.created_at, s.vault_path, s.original_path, s.file_size, s.workspace_name
             FROM sessions s
             JOIN sessions_fts fts ON s.id = fts.id
             WHERE sessions_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
            Ok(SessionMetadataRow {
                id: row.get(0)?,
                source: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                vault_path: row.get(4)?,
                original_path: row.get(5)?,
                file_size: row.get(6)?,
                workspace_name: row.get(7)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?.into());
        }

        Ok(sessions)
    }

    /// Filter sessions by source (vscode-copilot, cursor, etc.)
    pub fn filter_by_source(&self, source: &str, limit: usize) -> Result<Vec<SessionMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source, title, created_at, vault_path, original_path, file_size, workspace_name
             FROM sessions
             WHERE source = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![source, limit as i64], |row| {
            Ok(SessionMetadataRow {
                id: row.get(0)?,
                source: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                vault_path: row.get(4)?,
                original_path: row.get(5)?,
                file_size: row.get(6)?,
                workspace_name: row.get(7)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?.into());
        }

        Ok(sessions)
    }

    /// Filter sessions by workspace name
    pub fn filter_by_workspace(
        &self,
        workspace: &str,
        limit: usize,
    ) -> Result<Vec<SessionMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source, title, created_at, vault_path, original_path, file_size, workspace_name
             FROM sessions
             WHERE workspace_name = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![workspace, limit as i64], |row| {
            Ok(SessionMetadataRow {
                id: row.get(0)?,
                source: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                vault_path: row.get(4)?,
                original_path: row.get(5)?,
                file_size: row.get(6)?,
                workspace_name: row.get(7)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?.into());
        }

        Ok(sessions)
    }

    /// Get session by ID
    pub fn get(&self, id: &str) -> Result<Option<SessionMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source, title, created_at, vault_path, original_path, file_size, workspace_name
             FROM sessions
             WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(params![id], |row| {
            Ok(SessionMetadataRow {
                id: row.get(0)?,
                source: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                vault_path: row.get(4)?,
                original_path: row.get(5)?,
                file_size: row.get(6)?,
                workspace_name: row.get(7)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?.into())),
            None => Ok(None),
        }
    }

    /// Delete session from index
    pub fn delete(&self, id: &str) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    /// Count total number of sessions
    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Count number of sessions by source
    pub fn count_by_source(&self, source: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE source = ?1",
            params![source],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Get list of all workspaces
    pub fn list_workspaces(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT workspace_name FROM sessions WHERE workspace_name IS NOT NULL ORDER BY workspace_name",
        )?;

        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            Ok(name)
        })?;

        let mut workspaces = Vec::new();
        for row in rows {
            workspaces.push(row?);
        }

        Ok(workspaces)
    }

    /// Get list of all sources
    pub fn list_sources(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT source FROM sessions ORDER BY source")?;

        let rows = stmt.query_map([], |row| {
            let source: String = row.get(0)?;
            Ok(source)
        })?;

        let mut sources = Vec::new();
        for row in rows {
            sources.push(row?);
        }

        Ok(sources)
    }

    /// Check if a session exists in the index
    pub fn exists(&self, id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

/// Intermediate struct to map from SQLite row
struct SessionMetadataRow {
    id: String,
    source: String,
    title: Option<String>,
    created_at: Option<String>,
    vault_path: String,
    original_path: String,
    file_size: i64,
    workspace_name: Option<String>,
}

impl From<SessionMetadataRow> for SessionMetadata {
    fn from(row: SessionMetadataRow) -> Self {
        let created_at = row
            .created_at
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        SessionMetadata {
            id: row.id,
            source: row.source,
            title: row.title,
            created_at,
            vault_path: PathBuf::from(row.vault_path),
            original_path: PathBuf::from(row.original_path),
            file_size: row.file_size as u64,
            workspace_name: row.workspace_name,
            ide_origin: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_metadata(id: &str) -> SessionMetadata {
        SessionMetadata {
            id: id.to_string(),
            source: "vscode-copilot".to_string(),
            title: Some(format!("Test Session {}", id)),
            created_at: Some(Utc::now()),
            vault_path: PathBuf::from(format!("/vault/vscode-copilot/{}.json", id)),
            original_path: PathBuf::from(format!("/original/{}.json", id)),
            file_size: 1024,
            workspace_name: Some("test-project".to_string()),
            ide_origin: None,
        }
    }

    #[test]
    fn test_upsert_and_get() -> Result<()> {
        let index = SessionIndex::open_in_memory()?;
        let metadata = create_test_metadata("session-1");

        index.upsert(&metadata)?;

        let retrieved = index.get("session-1")?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, "session-1");
        assert_eq!(retrieved.source, "vscode-copilot");

        Ok(())
    }

    #[test]
    fn test_list_and_count() -> Result<()> {
        let mut index = SessionIndex::open_in_memory()?;
        let sessions: Vec<SessionMetadata> = (1..=5)
            .map(|i| create_test_metadata(&format!("s{}", i)))
            .collect();

        index.upsert_batch(&sessions)?;

        assert_eq!(index.count()?, 5);

        let listed = index.list(10, 0)?;
        assert_eq!(listed.len(), 5);

        Ok(())
    }

    #[test]
    fn test_search() -> Result<()> {
        let mut index = SessionIndex::open_in_memory()?;

        let mut m1 = create_test_metadata("s1");
        m1.title = Some("FastAPI middleware optimization".to_string());

        let mut m2 = create_test_metadata("s2");
        m2.title = Some("React component refactoring".to_string());

        index.upsert_batch(&[m1, m2])?;

        let results = index.search("FastAPI", 10)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "s1");

        Ok(())
    }

    #[test]
    fn test_delete() -> Result<()> {
        let index = SessionIndex::open_in_memory()?;
        let metadata = create_test_metadata("to-delete");

        index.upsert(&metadata)?;
        assert!(index.exists("to-delete")?);

        index.delete("to-delete")?;
        assert!(!index.exists("to-delete")?);

        // Verified deletion
        Ok(())
    }
}
