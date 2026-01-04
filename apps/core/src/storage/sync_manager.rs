//! SyncManager - Extract and apply CRDT changes for multi-machine sync.
//!
//! This module provides functionality to:
//! - Extract local changes to send to remote
//! - Apply changes received from remote
//! - Track sync state (last synced db_version)

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// A single change from crsql_changes virtual table.
/// Represents a column-level change with CRDT metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdtChange {
    pub table: String,
    pub pk: Vec<u8>,
    pub cid: String,
    pub val: Option<String>,
    pub col_version: i64,
    pub db_version: i64,
    pub site_id: Vec<u8>,
    pub cl: i64,
    pub seq: i64,
}

/// Changeset containing multiple changes for sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changeset {
    pub changes: Vec<CrdtChange>,
    pub from_db_version: i64,
    pub to_db_version: i64,
}

/// Get current db_version from cr-sqlite.
pub fn get_db_version(conn: &Connection) -> Result<i64> {
    let version: i64 = conn
        .query_row("SELECT crsql_db_version()", [], |row| row.get(0))
        .context("Failed to get db_version")?;
    Ok(version)
}

/// Get last synced db_version from sync_state table.
pub fn get_last_synced_version(conn: &Connection) -> Result<i64> {
    let version: i64 = conn
        .query_row(
            "SELECT last_synced_db_version FROM sync_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .context("Failed to get last synced version")?;
    Ok(version)
}

/// Update last synced db_version in sync_state table.
pub fn set_last_synced_version(conn: &Connection, version: i64) -> Result<()> {
    conn.execute(
        "UPDATE sync_state SET last_synced_db_version = ? WHERE id = 1",
        params![version],
    )?;
    Ok(())
}

/// Extract local changes since last sync.
/// Returns a Changeset containing all changes after `since_version`.
pub fn get_local_changes(conn: &Connection, since_version: i64) -> Result<Changeset> {
    let current_version = get_db_version(conn)?;

    let mut stmt = conn.prepare(
        r#"
        SELECT "table", "pk", "cid", "val", "col_version", "db_version", "site_id", "cl", "seq"
        FROM crsql_changes
        WHERE db_version > ? AND site_id = crsql_site_id()
        ORDER BY db_version, seq
        "#,
    )?;

    let changes = stmt
        .query_map(params![since_version], |row| {
            Ok(CrdtChange {
                table: row.get(0)?,
                pk: row.get(1)?,
                cid: row.get(2)?,
                val: row.get(3)?,
                col_version: row.get(4)?,
                db_version: row.get(5)?,
                site_id: row.get(6)?,
                cl: row.get(7)?,
                seq: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect changes")?;

    Ok(Changeset {
        changes,
        from_db_version: since_version,
        to_db_version: current_version,
    })
}

/// Apply changes received from remote.
/// Uses crsql_changes virtual table to merge CRDT changes.
pub fn apply_remote_changes(conn: &Connection, changeset: &Changeset) -> Result<usize> {
    let mut applied = 0;

    for change in &changeset.changes {
        conn.execute(
            r#"
            INSERT INTO crsql_changes ("table", "pk", "cid", "val", "col_version", "db_version", "site_id", "cl", "seq")
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            params![
                change.table,
                change.pk,
                change.cid,
                change.val,
                change.col_version,
                change.db_version,
                change.site_id,
                change.cl,
                change.seq,
            ],
        )?;
        applied += 1;
    }

    Ok(applied)
}

/// Serialize changeset to JSON for storage/transfer.
pub fn serialize_changeset(changeset: &Changeset) -> Result<Vec<u8>> {
    serde_json::to_vec(changeset).context("Failed to serialize changeset")
}

/// Deserialize changeset from JSON.
pub fn deserialize_changeset(data: &[u8]) -> Result<Changeset> {
    serde_json::from_slice(data).context("Failed to deserialize changeset")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_changeset() {
        let changeset = Changeset {
            changes: vec![CrdtChange {
                table: "sessions".to_string(),
                pk: vec![1, 2, 3],
                cid: "title".to_string(),
                val: Some("Test Session".to_string()),
                col_version: 1,
                db_version: 1,
                site_id: vec![4, 5, 6],
                cl: 1,
                seq: 0,
            }],
            from_db_version: 0,
            to_db_version: 1,
        };

        let serialized = serialize_changeset(&changeset).unwrap();
        let deserialized = deserialize_changeset(&serialized).unwrap();

        assert_eq!(deserialized.changes.len(), 1);
        assert_eq!(deserialized.changes[0].table, "sessions");
        assert_eq!(deserialized.from_db_version, 0);
        assert_eq!(deserialized.to_db_version, 1);
    }
}
