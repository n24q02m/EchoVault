//! Sync module - Synchronize vault with cloud storage.
//!
//! This module contains:
//! - SyncProvider trait for abstraction
//! - Rclone provider (supports 40+ cloud services)

pub mod provider;
pub mod rclone;

pub use provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
pub use rclone::RcloneProvider;
