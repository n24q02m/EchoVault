//! EchoVault Core Library
//!
//! Thư viện core cho EchoVault - "Black Box" cho lịch sử chat AI.
//! Cung cấp các chức năng:
//! - Trích xuất chat sessions từ các IDE (VS Code Copilot, Antigravity, etc.)
//! - Đồng bộ với cloud storage qua Rclone (Google Drive, Dropbox, OneDrive, S3, etc.)
//!
//! Nguyên tắc quan trọng: Lưu trữ raw JSON gốc, không format/transform data.

pub mod config;
pub mod extractors;
pub mod storage;
pub mod sync;
pub mod utils;
pub mod vault;

// Re-export main types
pub use config::Config;
pub use extractors::Extractor;
pub use storage::SessionIndex;
pub use sync::{AuthStatus, PullResult, PushResult, RcloneProvider, SyncOptions, SyncProvider};
pub use vault::VaultMetadata;
