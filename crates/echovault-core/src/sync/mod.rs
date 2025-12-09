//! Sync module - Đồng bộ vault với cloud storage.
//!
//! Module này chứa:
//! - SyncProvider trait cho abstraction
//! - Rclone provider (hỗ trợ 40+ cloud services)

pub mod provider;
pub mod rclone;

pub use provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
pub use rclone::RcloneProvider;
