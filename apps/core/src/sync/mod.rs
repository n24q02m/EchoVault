//! Sync module - Synchronize vault with cloud storage.
//!
//! This module contains:
//! - SyncProvider trait for abstraction
//! - Rclone provider (supports 40+ cloud services)
//! - Local provider (for local backups/sync)

pub mod local;
pub mod provider;
pub mod rclone;

pub use local::LocalProvider;
pub use provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
pub use rclone::RcloneProvider;
