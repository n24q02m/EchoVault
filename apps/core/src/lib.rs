//! EchoVault Core Library
//!
//! Core library for EchoVault - "Black Box" for AI chat history.
//! Provides the following capabilities:
//! - Extract chat sessions from various IDEs (VS Code Copilot, Cursor, Cline, Antigravity, etc.)
//! - Parse raw files into clean structured Markdown conversations
//! - Intercept API traffic via MITM proxy (feature-gated: `interceptor`)
//! - Sync with Google Drive via Rclone
//!
//! Pipeline: Extract (raw copy) -> Parse (structured Markdown) -> Embed (semantic vectors) -> Search/MCP

pub mod config;
#[cfg(feature = "embedding")]
pub mod embedding;
pub mod extractors;
#[cfg(feature = "interceptor")]
pub mod interceptor;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod parsers;
pub mod storage;
pub mod sync;
pub mod utils;
pub mod vault;
pub mod watcher;

// Re-export main types
pub use config::Config;
pub use extractors::{all_extractors, Extractor, ExtractorKind};
pub use parsers::{ParsedConversation, Parser};
pub use storage::SessionIndex;
pub use sync::{
    AuthStatus, LocalProvider, PullResult, PushResult, RcloneProvider, SyncOptions, SyncProvider,
};
pub use vault::VaultMetadata;
pub use watcher::FileWatcher;
