//! VaultDb - SQLite database for vault synchronization.
//!
//! This module handles the vault.db file which is synced across machines.
//! It provides conflict resolution for multi-machine sync scenarios using
//! cr-sqlite CRDT (Conflict-free Replicated Data Types).

use anyhow::{Context, Result};
use rusqlite::{params, Connection, LoadExtensionGuard, OptionalExtension};
use std::path::{Path, PathBuf};
use tracing::info;

/// Find cr-sqlite extension binary path.
/// Looks in: bundled (same dir as exe), development path, system paths.
fn find_crsqlite_path() -> Option<PathBuf> {
    // Try bundled path first (Tauri sidecar)
    if let Ok(exe_path) = std::env::current_exe() {
        let real_exe = exe_path.canonicalize().unwrap_or(exe_path);
        if let Some(exe_dir) = real_exe.parent() {
            let ext = if cfg!(windows) {
                ".dll"
            } else if cfg!(target_os = "macos") {
                ".dylib"
            } else {
                ".so"
            };
            let bundled = exe_dir.join(format!("crsqlite{}", ext));
            if bundled.exists() {
                return Some(bundled);
            }
        }
    }

    // Try development paths
    let dev_paths = [
        "apps/tauri/binaries/crsqlite-x86_64-unknown-linux-gnu.so",
        "apps/tauri/binaries/crsqlite-aarch64-unknown-linux-gnu.so",
        "apps/tauri/binaries/crsqlite-x86_64-apple-darwin.dylib",
        "apps/tauri/binaries/crsqlite-aarch64-apple-darwin.dylib",
        "apps/tauri/binaries/crsqlite-x86_64-pc-windows-msvc.dll",
    ];
    for path in dev_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // Try system paths (Docker/Linux installation)
    let system_paths = [
        "/usr/local/lib/crsqlite.so",
        "/usr/lib/crsqlite.so",
        "/usr/local/lib/crsqlite.dylib",
    ];
    for path in system_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    None
}

/// Get a unique machine identifier for this installation.
fn get_machine_id() -> String {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    // Generate a short random suffix for uniqueness
    let random_suffix: String = uuid::Uuid::new_v4().to_string()[..8].to_string();
    format!("{}-{}", hostname, random_suffix)
}

/// Lazy-initialized machine ID (generated once per process).
fn machine_id() -> &'static str {
    use std::sync::OnceLock;
    static MACHINE_ID: OnceLock<String> = OnceLock::new();
    MACHINE_ID.get_or_init(get_machine_id)
}

/// SQLite database for vault synchronization.
///
/// This database is synced across machines via rclone.
/// It uses WAL mode for better concurrent access and
/// implements conflict resolution based on modification time.
pub struct VaultDb {
    conn: Connection,
}

/// Result of an upsert operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpsertResult {
    /// New session was inserted.
    Inserted,
    /// Existing session was updated (newer version).
    Updated,
    /// No change (same or older version).
    NoChange,
    /// Skipped because remote version is newer.
    Skipped { reason: String },
}

/// A session entry for the vault database.
#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub id: String,
    pub source: String,
    pub mtime: u64,
    pub file_size: u64,
    pub title: Option<String>,
    pub workspace_name: Option<String>,
    pub created_at: Option<String>,
    pub vault_path: String,
    pub original_path: String,
}

impl VaultDb {
    /// Open or create the vault database with cr-sqlite CRDT support.
    pub fn open(vault_dir: &Path) -> Result<Self> {
        let db_path = vault_dir.join("vault.db");

        // Create directory if needed
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Cannot open vault database: {}", db_path.display()))?;

        // Enable WAL mode for concurrent access
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 5000;
        ",
        )?;

        // Load cr-sqlite extension for CRDT support
        if let Some(crsqlite_path) = find_crsqlite_path() {
            info!("[VaultDb] Loading cr-sqlite from: {:?}", crsqlite_path);
            unsafe {
                let _guard =
                    LoadExtensionGuard::new(&conn).context("Failed to enable extension loading")?;
                conn.load_extension(&crsqlite_path, Some("sqlite3_crsqlite_init"))
                    .with_context(|| {
                        format!("Failed to load cr-sqlite from: {}", crsqlite_path.display())
                    })?;
            }
            info!("[VaultDb] cr-sqlite loaded successfully");
        } else {
            info!("[VaultDb] cr-sqlite not found, running without CRDT support");
        }

        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Open database in memory (for testing).
    #[allow(dead_code)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Check if cr-sqlite extension is loaded.
    fn is_crsqlite_loaded(&self) -> bool {
        // Try calling crsql_site_id() - this will only work if extension is loaded
        self.conn
            .query_row("SELECT crsql_site_id()", [], |_| Ok(()))
            .is_ok()
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<()> {
        // Create tables first
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY NOT NULL,
                source TEXT NOT NULL,
                machine_id TEXT NOT NULL,
                mtime INTEGER NOT NULL,
                file_size INTEGER NOT NULL,
                last_synced INTEGER NOT NULL,
                title TEXT,
                workspace_name TEXT,
                created_at TEXT,
                vault_path TEXT NOT NULL,
                original_path TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_source ON sessions(source);
            CREATE INDEX IF NOT EXISTS idx_sessions_machine ON sessions(machine_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_mtime ON sessions(mtime);

            CREATE TABLE IF NOT EXISTS sync_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                machine_id TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                action TEXT NOT NULL,
                details TEXT
            );

            -- Track sync state for incremental sync
            CREATE TABLE IF NOT EXISTS sync_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                last_synced_db_version INTEGER NOT NULL DEFAULT 0
            );
            INSERT OR IGNORE INTO sync_state (id, last_synced_db_version) VALUES (1, 0);
        ",
        )?;

        // Upgrade sessions table to CRR if cr-sqlite is loaded
        if self.is_crsqlite_loaded() {
            info!("[VaultDb] Upgrading sessions table to CRR...");
            // crsql_as_crr is idempotent - safe to call multiple times
            self.conn.execute("SELECT crsql_as_crr('sessions')", [])?;
            info!("[VaultDb] Sessions table upgraded to CRR");
        }

        Ok(())
    }

    /// Get the current machine ID.
    pub fn get_machine_id(&self) -> &str {
        machine_id()
    }

    /// Get reference to the underlying connection (for testing/advanced use).
    #[cfg(feature = "ci-sync-test")]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Upsert a session with conflict resolution (keep newest by mtime).
    pub fn upsert_session(&self, session: &SessionEntry) -> Result<UpsertResult> {
        // Check if session exists
        let existing: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT mtime, machine_id FROM sessions WHERE id = ?1",
                params![session.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let now = chrono::Utc::now().timestamp();

        match existing {
            Some((existing_mtime, _existing_machine)) => {
                if session.mtime as i64 > existing_mtime {
                    // New version is newer, update
                    self.conn.execute(
                        "UPDATE sessions SET
                            source = ?2, machine_id = ?3, mtime = ?4,
                            file_size = ?5, last_synced = ?6, title = ?7,
                            workspace_name = ?8, created_at = ?9,
                            vault_path = ?10, original_path = ?11
                         WHERE id = ?1",
                        params![
                            session.id,
                            session.source,
                            machine_id(),
                            session.mtime as i64,
                            session.file_size as i64,
                            now,
                            session.title,
                            session.workspace_name,
                            session.created_at,
                            session.vault_path,
                            session.original_path
                        ],
                    )?;
                    Ok(UpsertResult::Updated)
                } else if (session.mtime as i64) < existing_mtime {
                    // Existing is newer, skip
                    Ok(UpsertResult::Skipped {
                        reason: "Remote version is newer".into(),
                    })
                } else {
                    // Same mtime, no change
                    Ok(UpsertResult::NoChange)
                }
            }
            None => {
                // Insert new session
                self.conn.execute(
                    "INSERT INTO sessions
                        (id, source, machine_id, mtime, file_size, last_synced,
                         title, workspace_name, created_at, vault_path, original_path)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        session.id,
                        session.source,
                        machine_id(),
                        session.mtime as i64,
                        session.file_size as i64,
                        now,
                        session.title,
                        session.workspace_name,
                        session.created_at,
                        session.vault_path,
                        session.original_path
                    ],
                )?;
                Ok(UpsertResult::Inserted)
            }
        }
    }

    /// Upsert multiple sessions in a transaction.
    pub fn upsert_batch(&mut self, sessions: &[SessionEntry]) -> Result<BatchResult> {
        let tx = self.conn.transaction()?;
        let now = chrono::Utc::now().timestamp();

        let mut inserted = 0;
        let mut updated = 0;
        let mut skipped = 0;

        for session in sessions {
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT mtime FROM sessions WHERE id = ?1",
                    params![session.id],
                    |row| row.get(0),
                )
                .optional()?;

            match existing {
                Some(existing_mtime) => {
                    if session.mtime as i64 > existing_mtime {
                        tx.execute(
                            "UPDATE sessions SET
                                source = ?2, machine_id = ?3, mtime = ?4,
                                file_size = ?5, last_synced = ?6, title = ?7,
                                workspace_name = ?8, created_at = ?9,
                                vault_path = ?10, original_path = ?11
                             WHERE id = ?1",
                            params![
                                session.id,
                                session.source,
                                machine_id(),
                                session.mtime as i64,
                                session.file_size as i64,
                                now,
                                session.title,
                                session.workspace_name,
                                session.created_at,
                                session.vault_path,
                                session.original_path
                            ],
                        )?;
                        updated += 1;
                    } else {
                        skipped += 1;
                    }
                }
                None => {
                    tx.execute(
                        "INSERT INTO sessions
                            (id, source, machine_id, mtime, file_size, last_synced,
                             title, workspace_name, created_at, vault_path, original_path)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                        params![
                            session.id,
                            session.source,
                            machine_id(),
                            session.mtime as i64,
                            session.file_size as i64,
                            now,
                            session.title,
                            session.workspace_name,
                            session.created_at,
                            session.vault_path,
                            session.original_path
                        ],
                    )?;
                    inserted += 1;
                }
            }
        }

        tx.commit()?;

        Ok(BatchResult {
            inserted,
            updated,
            skipped,
        })
    }

    /// Get all sessions from the database.
    pub fn get_all_sessions(&self) -> Result<Vec<SessionEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source, mtime, file_size, title, workspace_name,
                    created_at, vault_path, original_path
             FROM sessions
             ORDER BY mtime DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(SessionEntry {
                id: row.get(0)?,
                source: row.get(1)?,
                mtime: row.get::<_, i64>(2)? as u64,
                file_size: row.get::<_, i64>(3)? as u64,
                title: row.get(4)?,
                workspace_name: row.get(5)?,
                created_at: row.get(6)?,
                vault_path: row.get(7)?,
                original_path: row.get(8)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }

        Ok(sessions)
    }

    /// Check if a session exists and get its mtime.
    pub fn get_session_mtime(&self, id: &str) -> Result<Option<u64>> {
        let mtime: Option<i64> = self
            .conn
            .query_row(
                "SELECT mtime FROM sessions WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;

        Ok(mtime.map(|m| m as u64))
    }

    /// Log a sync action.
    pub fn log_sync(&self, action: &str, details: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO sync_log (machine_id, timestamp, action, details)
             VALUES (?1, ?2, ?3, ?4)",
            params![machine_id(), now, action, details],
        )?;
        Ok(())
    }

    /// Get total count of sessions.
    pub fn count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Get count of sessions by source.
    pub fn count_by_source(&self, source: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE source = ?1",
            params![source],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

/// Result of a batch upsert operation.
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session(id: &str, mtime: u64) -> SessionEntry {
        SessionEntry {
            id: id.to_string(),
            source: "vscode-copilot".to_string(),
            mtime,
            file_size: 1024,
            title: Some(format!("Test Session {}", id)),
            workspace_name: Some("test-project".to_string()),
            created_at: Some("2024-12-24T10:00:00Z".to_string()),
            vault_path: format!("/vault/vscode-copilot/{}.json", id),
            original_path: format!("/original/{}.json", id),
        }
    }

    #[test]
    fn test_insert_new_session() -> Result<()> {
        let db = VaultDb::open_in_memory()?;
        let session = create_test_session("s1", 1000);

        let result = db.upsert_session(&session)?;
        assert_eq!(result, UpsertResult::Inserted);
        assert_eq!(db.count()?, 1);

        Ok(())
    }

    #[test]
    fn test_update_newer_session() -> Result<()> {
        let db = VaultDb::open_in_memory()?;

        // Insert old version
        let old = create_test_session("s1", 1000);
        db.upsert_session(&old)?;

        // Update with newer version
        let new = create_test_session("s1", 2000);
        let result = db.upsert_session(&new)?;
        assert_eq!(result, UpsertResult::Updated);

        // Verify mtime was updated
        let mtime = db.get_session_mtime("s1")?;
        assert_eq!(mtime, Some(2000));

        Ok(())
    }

    #[test]
    fn test_skip_older_session() -> Result<()> {
        let db = VaultDb::open_in_memory()?;

        // Insert newer version first
        let new = create_test_session("s1", 2000);
        db.upsert_session(&new)?;

        // Try to update with older version
        let old = create_test_session("s1", 1000);
        let result = db.upsert_session(&old)?;
        assert!(matches!(result, UpsertResult::Skipped { .. }));

        // Verify mtime was not changed
        let mtime = db.get_session_mtime("s1")?;
        assert_eq!(mtime, Some(2000));

        Ok(())
    }

    #[test]
    fn test_batch_upsert() -> Result<()> {
        let mut db = VaultDb::open_in_memory()?;

        let sessions = vec![
            create_test_session("s1", 1000),
            create_test_session("s2", 2000),
            create_test_session("s3", 3000),
        ];

        let result = db.upsert_batch(&sessions)?;
        assert_eq!(result.inserted, 3);
        assert_eq!(result.updated, 0);
        assert_eq!(result.skipped, 0);
        assert_eq!(db.count()?, 3);

        Ok(())
    }

    #[test]
    fn test_get_all_sessions() -> Result<()> {
        let mut db = VaultDb::open_in_memory()?;

        let sessions = vec![
            create_test_session("s1", 1000),
            create_test_session("s2", 2000),
        ];

        db.upsert_batch(&sessions)?;

        let all = db.get_all_sessions()?;
        assert_eq!(all.len(), 2);
        // Should be ordered by mtime DESC
        assert_eq!(all[0].id, "s2");
        assert_eq!(all[1].id, "s1");

        Ok(())
    }
}
