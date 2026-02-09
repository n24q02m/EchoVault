//! SyncProvider trait - Abstraction for sync backend.
//!
//! This trait provides an interface for syncing with Google Drive via Rclone.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Options for sync operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOptions {
    /// Whether to encrypt data before sync
    pub encrypt: bool,
    /// Whether to compress data before sync
    pub compress: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            encrypt: true,
            compress: true,
        }
    }
}

/// Result of a pull operation.
#[derive(Debug, Clone)]
pub struct PullResult {
    /// Whether changes were pulled
    pub has_changes: bool,
    /// Number of new files
    pub new_files: usize,
    /// Number of updated files
    pub updated_files: usize,
}

/// Result of a push operation.
#[derive(Debug, Clone)]
pub struct PushResult {
    /// Whether push was successful
    pub success: bool,
    /// Number of files pushed
    pub files_pushed: usize,
    /// Message (if any)
    pub message: Option<String>,
}

/// Authentication status.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthStatus {
    /// Not authenticated
    NotAuthenticated,
    /// Authentication pending (waiting for user action)
    Pending {
        user_code: String,
        verify_url: String,
    },
    /// Authenticated
    Authenticated,
    /// Authentication error
    Error(String),
}

/// Trait for all sync providers.
///
/// Each provider implements this trait to provide
/// sync capability with a specific backend.
pub trait SyncProvider: Send + Sync {
    /// Provider name (github, google_drive, s3)
    fn name(&self) -> &'static str;

    /// Check if authenticated
    fn is_authenticated(&self) -> bool;

    /// Get current auth status
    fn auth_status(&self) -> AuthStatus;

    /// Start authentication process.
    /// Returns AuthStatus::Pending if user action is needed.
    fn start_auth(&mut self) -> Result<AuthStatus>;

    /// Complete authentication (poll for OAuth device flow)
    fn complete_auth(&mut self) -> Result<AuthStatus>;

    /// Pull data from remote to vault
    fn pull(&self, vault_dir: &Path, options: &SyncOptions) -> Result<PullResult>;

    /// Push data from vault to remote
    fn push(&self, vault_dir: &Path, options: &SyncOptions) -> Result<PushResult>;

    /// Check if there are local changes to sync
    fn has_local_changes(&self, vault_dir: &Path) -> Result<bool>;

    /// Check if remote has new changes
    fn has_remote_changes(&self, vault_dir: &Path) -> Result<bool>;

    /// Enable encryption with password (if supported)
    fn enable_encryption(&mut self, _password: String) -> Result<()> {
        anyhow::bail!("Encryption not supported by this provider")
    }
}
