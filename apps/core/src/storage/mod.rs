//! Storage module - Manages raw JSON file storage and index metadata.
//!
//! This module contains:
//! - SQLite index for fast session search and filtering
//! - VaultDb for multi-machine sync with conflict resolution
//! - SyncManager for cr-sqlite CRDT sync support
//! - Utilities for vault directory management

pub mod index;
pub mod sync_manager;
pub mod vault_db;

#[cfg(feature = "ci-sync-test")]
pub mod sync_test;

pub use index::SessionIndex;
pub use sync_manager::{
    apply_remote_changes, deserialize_changeset, get_db_version, get_last_synced_version,
    get_local_changes, serialize_changeset, set_last_synced_version, Changeset, CrdtChange,
};
pub use vault_db::{BatchResult, SessionEntry, UpsertResult, VaultDb};
