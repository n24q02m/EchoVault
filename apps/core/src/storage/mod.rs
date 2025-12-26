//! Storage module - Manages raw JSON file storage and index metadata.
//!
//! This module contains:
//! - SQLite index for fast session search and filtering
//! - VaultDb for multi-machine sync with conflict resolution
//! - Utilities for vault directory management

pub mod index;
pub mod vault_db;

#[cfg(feature = "ci-sync-test")]
pub mod sync_test;

pub use index::SessionIndex;
pub use vault_db::{BatchResult, SessionEntry, UpsertResult, VaultDb};
