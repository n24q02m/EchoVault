//! Extractors module - Extract chat history from various IDEs.
//!
//! Principle: ONLY COPY raw files, DO NOT format/transform data.
//! This ensures no information loss when IDE changes format.

// NOTE: Antigravity artifacts now supported (conversations still encrypted)
pub mod antigravity;
pub mod cline;
pub mod cursor;
pub mod vscode_copilot;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Session file metadata (for indexing).
/// Contains only basic information, NOT content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session ID (usually filename)
    pub id: String,
    /// Source (vscode-copilot, cursor, cline, etc.)
    pub source: String,
    /// Title (if quickly extractable)
    pub title: Option<String>,
    /// Creation time (if quickly extractable)
    pub created_at: Option<DateTime<Utc>>,
    /// Path to raw file in vault
    pub vault_path: PathBuf,
    /// Original file path (for debugging)
    pub original_path: PathBuf,
    /// File size (bytes)
    pub file_size: u64,
    /// Workspace name (project name)
    pub workspace_name: Option<String>,
}

/// Information about a session file to copy.
#[derive(Debug, Clone)]
pub struct SessionFile {
    /// Path to source file
    pub source_path: PathBuf,
    /// Basic metadata
    pub metadata: SessionMetadata,
}

/// Trait for all extractors.
/// Extractors only find and copy files, DO NOT parse content in detail.
pub trait Extractor: Sync {
    /// Source name (vscode-copilot, cursor, etc.)
    fn source_name(&self) -> &'static str;

    /// Find all directories containing chat sessions.
    fn find_storage_locations(&self) -> Result<Vec<PathBuf>>;

    /// Get workspace name from location path.
    fn get_workspace_name(&self, location: &Path) -> String;

    /// List all session files in a location.
    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>>;

    /// Count sessions in a location (fast, no metadata parsing).
    fn count_sessions(&self, location: &Path) -> Result<usize>;

    /// Copy a session file to vault (incremental - only copy if new/changed).
    /// Returns Some(path) if file was copied, None if unchanged (skipped).
    fn copy_to_vault(&self, session: &SessionFile, vault_dir: &Path) -> Result<Option<PathBuf>> {
        // Create subdirectory by source
        let source_dir = vault_dir.join(self.source_name());
        std::fs::create_dir_all(&source_dir)?;

        // Keep original filename
        let filename = session
            .source_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let dest_path = source_dir.join(&filename);

        // Incremental: only copy if file is new or changed
        let should_copy = if dest_path.exists() {
            // Compare size and modified time
            let src_meta = session.source_path.metadata()?;
            let dest_meta = dest_path.metadata()?;

            let src_modified = src_meta.modified()?;
            let dest_modified = dest_meta.modified()?;

            // Copy if source is newer or size differs
            src_modified > dest_modified || src_meta.len() != dest_meta.len()
        } else {
            true // File doesn't exist, need to copy
        };

        if should_copy {
            std::fs::copy(&session.source_path, &dest_path)?;
            Ok(Some(dest_path))
        } else {
            Ok(None) // File unchanged
        }
    }
}
