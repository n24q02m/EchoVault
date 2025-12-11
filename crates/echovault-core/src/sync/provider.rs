//! SyncProvider trait - Abstraction cho sync backend.
//!
//! Trait này cung cấp interface để sync với Google Drive thông qua Rclone.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Các tuỳ chọn cho sync operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOptions {
    /// Có mã hoá dữ liệu trước khi sync không
    pub encrypt: bool,
    /// Có nén dữ liệu trước khi sync không
    pub compress: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            encrypt: true,
            compress: true,
        }
    }
}

/// Kết quả của pull operation
#[derive(Debug, Clone)]
pub struct PullResult {
    /// Có thay đổi được pull không
    pub has_changes: bool,
    /// Số files mới
    pub new_files: usize,
    /// Số files được cập nhật
    pub updated_files: usize,
}

/// Kết quả của push operation
#[derive(Debug, Clone)]
pub struct PushResult {
    /// Push thành công không
    pub success: bool,
    /// Số files được push
    pub files_pushed: usize,
    /// Message (nếu có)
    pub message: Option<String>,
}

/// Trạng thái xác thực
#[derive(Debug, Clone, PartialEq)]
pub enum AuthStatus {
    /// Chưa xác thực
    NotAuthenticated,
    /// Đang xác thực (chờ user action)
    Pending {
        user_code: String,
        verify_url: String,
    },
    /// Đã xác thực
    Authenticated,
    /// Lỗi xác thực
    Error(String),
}

/// Trait cho tất cả sync providers
///
/// Mỗi provider implement trait này để cung cấp khả năng
/// đồng bộ vault với một backend cụ thể.
pub trait SyncProvider: Send + Sync {
    /// Tên của provider (github, google_drive, s3)
    fn name(&self) -> &'static str;

    /// Kiểm tra xem đã xác thực chưa
    fn is_authenticated(&self) -> bool;

    /// Lấy trạng thái xác thực hiện tại
    fn auth_status(&self) -> AuthStatus;

    /// Bắt đầu quá trình xác thực
    /// Trả về AuthStatus::Pending nếu cần user action
    fn start_auth(&mut self) -> Result<AuthStatus>;

    /// Hoàn tất xác thực (poll cho OAuth device flow)
    fn complete_auth(&mut self) -> Result<AuthStatus>;

    /// Pull dữ liệu từ remote về vault
    fn pull(&self, vault_dir: &Path, options: &SyncOptions) -> Result<PullResult>;

    /// Push dữ liệu từ vault lên remote
    fn push(&self, vault_dir: &Path, options: &SyncOptions) -> Result<PushResult>;

    /// Kiểm tra xem có thay đổi cần sync không
    fn has_local_changes(&self, vault_dir: &Path) -> Result<bool>;

    /// Kiểm tra xem remote có thay đổi mới không
    fn has_remote_changes(&self, vault_dir: &Path) -> Result<bool>;
}
