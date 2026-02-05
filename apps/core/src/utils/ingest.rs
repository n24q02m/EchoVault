// Filtering logic for ingest_sessions

use crate::extractors::SessionFile;
use crate::storage::VaultDb;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;

/// Filter sessions that need processing (Sequential version for baseline)
/// This matches the original logic in commands.rs
#[cfg(test)]
pub fn filter_sessions_sequential(
    sessions: Vec<SessionFile>,
    vault_db: &VaultDb,
) -> Vec<(SessionFile, u64, u64)> {
    sessions
        .into_iter()
        .filter_map(|session| {
            let source_path = &session.metadata.original_path;
            let file_size = session.metadata.file_size;

            // Skip if source file doesn't exist
            let metadata = match fs::metadata(source_path) {
                Ok(m) => m,
                Err(_) => return None,
            };

            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Check against vault.db
            let should_process = match vault_db.get_session_mtime(&session.metadata.id) {
                Ok(Some(cached_mtime)) => mtime > cached_mtime,
                Ok(None) => true,
                Err(_) => true, // Process if we can't check
            };

            if should_process {
                Some((session, mtime, file_size))
            } else {
                None
            }
        })
        .collect()
}

/// Filter sessions that need processing (Parallel optimized version)
pub fn filter_sessions_parallel(
    sessions: Vec<SessionFile>,
    vault_db: &VaultDb,
) -> Vec<(SessionFile, u64, u64)> {
    // 1. Prefetch all session mtimes into a HashMap
    // This reduces N DB queries to 1 query.
    let existing_mtimes: HashMap<String, u64> = vault_db
        .get_all_session_mtimes()
        .unwrap_or_default()
        .into_iter()
        .collect();

    // 2. Parallel filter using pre-fetched data
    // fs::metadata is IO bound, so parallelism helps.
    // HashMap lookup is fast and CPU bound (but very cheap).
    sessions
        .into_par_iter() // Parallel iterator
        .filter_map(|session| {
            let source_path = &session.metadata.original_path;
            let file_size = session.metadata.file_size;

            // Skip if source file doesn't exist
            // This does IO check
            let metadata = match fs::metadata(source_path) {
                Ok(m) => m,
                Err(_) => return None,
            };

            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // Check against in-memory map
            let should_process = match existing_mtimes.get(&session.metadata.id) {
                Some(&cached_mtime) => mtime > cached_mtime,
                None => true,
            };

            if should_process {
                Some((session, mtime, file_size))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extractors::SessionMetadata;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper to create a dummy file
    fn create_dummy_file(dir: &std::path::Path, name: &str, mtime: u64) -> (PathBuf, SessionFile) {
        let path = dir.join(name);
        std::fs::write(&path, "dummy content").unwrap();

        // Set mtime
        filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(mtime as i64, 0))
            .unwrap();

        let session = SessionFile {
            source_path: path.clone(),
            metadata: SessionMetadata {
                id: name.to_string(),
                source: "test".to_string(),
                title: None,
                created_at: None,
                vault_path: PathBuf::from("vault").join(name),
                original_path: path.clone(),
                file_size: 13,
                workspace_name: None,
            },
        };
        (path, session)
    }

    #[test]
    fn benchmark_filtering() {
        let temp_dir = TempDir::new().unwrap();
        // Use in-memory DB to avoid disk I/O for DB, focusing on the query overhead difference
        let db = VaultDb::open_in_memory().unwrap();

        // Create 1000 dummy sessions
        let count = 1000;
        let mut sessions = Vec::new();

        println!("Setting up {} dummy files and DB entries...", count);
        for i in 0..count {
            let name = format!("session_{}", i);
            // File mtime = 2000
            let (_, session) = create_dummy_file(temp_dir.path(), &name, 2000);
            sessions.push(session);

            // DB mtime = 1000 (so file is newer, should process)
            // Insert into DB
            let entry = crate::storage::SessionEntry {
                id: name,
                source: "test".to_string(),
                mtime: 1000,
                file_size: 13,
                title: None,
                workspace_name: None,
                created_at: None,
                vault_path: "vault".to_string(),
                original_path: "orig".to_string(),
            };
            db.upsert_session(&entry).unwrap();
        }

        println!("Starting benchmark...");

        // Measure Sequential
        let start = std::time::Instant::now();
        let res_seq = filter_sessions_sequential(sessions.clone(), &db);
        let duration_seq = start.elapsed();
        println!("Sequential time: {:?}", duration_seq);
        assert_eq!(res_seq.len(), count);

        // Measure Parallel
        let start = std::time::Instant::now();
        let res_par = filter_sessions_parallel(sessions.clone(), &db);
        let duration_par = start.elapsed();
        println!("Parallel time:   {:?}", duration_par);
        assert_eq!(res_par.len(), count);

        let speedup = duration_seq.as_secs_f64() / duration_par.as_secs_f64();
        println!("Speedup: {:.2}x", speedup);

        // Ensure correctness - result should be identical
        // (Order might differ, but length is checked. filter_sessions_sequential preserves order, parallel might not)

        // Also test case where update is NOT needed
        let name_skip = "session_skip";
        let (_, session_skip) = create_dummy_file(temp_dir.path(), name_skip, 1000);
        let entry_skip = crate::storage::SessionEntry {
            id: name_skip.to_string(),
            source: "test".to_string(),
            mtime: 2000, // DB newer
            file_size: 13,
            title: None,
            workspace_name: None,
            created_at: None,
            vault_path: "vault".to_string(),
            original_path: "orig".to_string(),
        };
        db.upsert_session(&entry_skip).unwrap();

        let sessions_mixed = vec![session_skip];
        let res_par_skip = filter_sessions_parallel(sessions_mixed, &db);
        assert_eq!(res_par_skip.len(), 0, "Should skip up-to-date session");
    }
}
