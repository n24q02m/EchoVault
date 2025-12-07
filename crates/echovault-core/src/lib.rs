//! EchoVault Core Library
//!
//! Thư viện core cho EchoVault - "Black Box" cho lịch sử chat AI.
//! Cung cấp các chức năng:
//! - Trích xuất chat sessions từ các IDE (VS Code Copilot, Antigravity, etc.)
//! - Mã hóa/giải mã dữ liệu với AES-256-GCM
//! - Đồng bộ với nhiều providers (GitHub, Google Drive, S3)
//!
//! Nguyên tắc quan trọng: Lưu trữ raw JSON gốc, không format/transform data.

pub mod config;
pub mod crypto;
pub mod extractors;
pub mod storage;
pub mod sync;
pub mod utils;
pub mod vault;
pub mod watcher;

// Re-export main types
pub use config::Config;
pub use crypto::Encryptor;
pub use extractors::Extractor;
pub use storage::SessionIndex;
pub use sync::GitSync;
pub use sync::{AuthStatus, GitHubProvider, PullResult, PushResult, SyncOptions, SyncProvider};
pub use vault::{verify_passphrase, VaultMetadata};
pub use watcher::{get_ide_storage_paths, FileWatcher};
