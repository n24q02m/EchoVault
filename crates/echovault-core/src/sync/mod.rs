//! Sync module - Đồng bộ vault với nhiều providers.
//!
//! Module này chứa:
//! - SyncProvider trait cho abstraction
//! - Git operations (GitHub provider)
//! - GitHub OAuth Device Flow cho authentication
//! - Encryption trước khi push

pub mod git;
pub mod github;
pub mod oauth;
pub mod provider;

pub use git::GitSync;
pub use github::GitHubProvider;
pub use provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
