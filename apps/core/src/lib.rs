//! EchoVault Core Library
//!
//! Core library for EchoVault - "Black Box" for AI chat history.
//! Provides the following capabilities:
//! - Extract chat sessions from various IDEs (VS Code Copilot, Cursor, Cline, Antigravity)
//! - Sync with Google Drive via Rclone
//!
//! Important principle: Store raw JSON files without transformation.

pub mod config;
pub mod extractors;
pub mod storage;
pub mod sync;
pub mod utils;
pub mod vault;
pub mod watcher;

// Re-export main types
pub use config::Config;
pub use extractors::Extractor;
pub use storage::SessionIndex;
pub use sync::{AuthStatus, PullResult, PushResult, RcloneProvider, SyncOptions, SyncProvider};
pub use vault::VaultMetadata;
pub use watcher::FileWatcher;
