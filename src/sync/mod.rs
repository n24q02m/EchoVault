//! Sync module - Đồng bộ vault với GitHub qua Git.
//!
//! Module này chứa:
//! - Git operations (init, commit, push, pull)
//! - GitHub OAuth Device Flow cho authentication
//! - Encryption trước khi push

pub mod git;
pub mod oauth;

pub use git::GitSync;
