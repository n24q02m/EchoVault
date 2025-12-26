//! CI Sync Tests - Multi-machine sync simulation for GitHub Actions.
//!
//! These tests are designed to run in CI to verify conflict resolution
//! when multiple machines sync to the same vault.
//!
//! Run with: `cargo test --features ci-sync-test -- --ignored`

use crate::storage::vault_db::{SessionEntry, UpsertResult, VaultDb};
use anyhow::Result;
use std::path::Path;

/// Test helper: create a session entry for testing.
fn create_test_session(id: &str, source: &str, mtime: u64) -> SessionEntry {
    SessionEntry {
        id: id.to_string(),
        source: source.to_string(),
        mtime,
        file_size: 1024,
        title: Some(format!("Test Session {}", id)),
        workspace_name: Some("test-project".to_string()),
        created_at: Some("2024-12-26T10:00:00Z".to_string()),
        vault_path: format!("/vault/{}/{}.json", source, id),
        original_path: format!("/original/{}.json", id),
    }
}

/// Get vault directory path for CI tests.
fn get_ci_vault_path() -> std::path::PathBuf {
    // Use current directory's vault folder (set by CI workflow)
    std::env::current_dir().unwrap().join("vault")
}

#[cfg(test)]
mod ci_sync_tests {
    use super::*;

    /// Machine A: Create vault and insert initial sessions.
    /// This runs first on Ubuntu.
    #[test]
    #[ignore] // Only run in CI with --ignored flag
    fn sync_test_machine_a_create() {
        let vault_path = get_ci_vault_path();
        std::fs::create_dir_all(&vault_path).expect("Failed to create vault dir");

        let db = VaultDb::open(&vault_path).expect("Failed to open VaultDb");

        // Insert sessions with mtime=1000 (older)
        let sessions = vec![
            create_test_session("session-001", "vscode-copilot", 1000),
            create_test_session("session-002", "cursor", 1000),
            create_test_session("session-003", "cline", 1000),
        ];

        for session in &sessions {
            let result = db.upsert_session(session).expect("Failed to upsert");
            assert_eq!(result, UpsertResult::Inserted);
        }

        assert_eq!(db.count().unwrap(), 3);
        println!("[Machine A] Created vault with 3 sessions (mtime=1000)");
    }

    /// Machine B: Download vault from A, insert newer sessions.
    /// This runs on Windows after Machine A.
    #[test]
    #[ignore]
    fn sync_test_machine_b_update() {
        let vault_path = get_ci_vault_path();

        let db = VaultDb::open(&vault_path).expect("Failed to open VaultDb");

        // Verify we have sessions from Machine A
        let initial_count = db.count().unwrap();
        assert_eq!(initial_count, 3, "Should have 3 sessions from Machine A");

        // Insert newer versions of same sessions (mtime=2000)
        let sessions = vec![
            create_test_session("session-001", "vscode-copilot", 2000),
            create_test_session("session-002", "cursor", 2000),
        ];

        for session in &sessions {
            let result = db.upsert_session(session).expect("Failed to upsert");
            assert_eq!(
                result,
                UpsertResult::Updated,
                "Should update because 2000 > 1000"
            );
        }

        // Also add a new session only on Machine B
        let new_session = create_test_session("session-004", "antigravity", 2000);
        let result = db.upsert_session(&new_session).expect("Failed to upsert");
        assert_eq!(result, UpsertResult::Inserted);

        assert_eq!(db.count().unwrap(), 4);
        println!("[Machine B] Updated 2 sessions (mtime=2000), added 1 new session");
    }

    /// Machine A: Download vault from B, verify conflict resolution.
    /// This runs on Ubuntu after Machine B.
    #[test]
    #[ignore]
    fn sync_test_verify_resolution() {
        let vault_path = get_ci_vault_path();

        let db = VaultDb::open(&vault_path).expect("Failed to open VaultDb");

        // Verify we have 4 sessions (3 original + 1 from Machine B)
        assert_eq!(db.count().unwrap(), 4, "Should have 4 sessions total");

        // Try to insert older versions (mtime=1500) - should be skipped
        let old_sessions = vec![
            create_test_session("session-001", "vscode-copilot", 1500),
            create_test_session("session-002", "cursor", 1500),
        ];

        for session in &old_sessions {
            let result = db.upsert_session(session).expect("Failed to upsert");
            assert!(
                matches!(result, UpsertResult::Skipped { .. }),
                "Should skip because 1500 < 2000"
            );
        }

        // Verify final mtime values
        let mtime_001 = db.get_session_mtime("session-001").unwrap();
        let mtime_002 = db.get_session_mtime("session-002").unwrap();
        let mtime_003 = db.get_session_mtime("session-003").unwrap();
        let mtime_004 = db.get_session_mtime("session-004").unwrap();

        assert_eq!(mtime_001, Some(2000), "session-001 should have mtime=2000");
        assert_eq!(mtime_002, Some(2000), "session-002 should have mtime=2000");
        assert_eq!(
            mtime_003,
            Some(1000),
            "session-003 should still have mtime=1000"
        );
        assert_eq!(
            mtime_004,
            Some(2000),
            "session-004 from Machine B should exist"
        );

        println!("[Verify] Conflict resolution correct! Newer versions preserved.");
        println!("  - session-001: mtime=2000 (from Machine B)");
        println!("  - session-002: mtime=2000 (from Machine B)");
        println!("  - session-003: mtime=1000 (unchanged)");
        println!("  - session-004: mtime=2000 (new from Machine B)");
    }
}
