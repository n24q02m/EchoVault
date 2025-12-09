//! Sync module - Đồng bộ vault với nhiều providers.
//!
//! Module này chứa:
//! - SyncProvider trait cho abstraction
//! - Git operations (GitHub provider)
//! - Google Drive provider
//! - OAuth Device Flow cho authentication

pub mod git;
pub mod github;
pub mod google_drive;
pub mod oauth;
pub mod provider;

pub use git::GitSync;
pub use github::GitHubProvider;
pub use google_drive::GoogleDriveProvider;
pub use oauth::{load_credentials_from_file, save_credentials_to_file, OAuthCredentials};
pub use provider::{AuthStatus, PullResult, PushResult, SyncOptions, SyncProvider};
