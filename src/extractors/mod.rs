//! Extractors module - Trích xuất chat history từ các IDE khác nhau.
//!
//! Nguyên tắc: CHỈ COPY raw files, KHÔNG format/transform data.
//! Điều này đảm bảo không mất thông tin khi IDE thay đổi format.

pub mod vscode_copilot;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Metadata của một session file (cho index)
/// Chỉ chứa thông tin cơ bản, KHÔNG chứa nội dung
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// ID duy nhất của session (thường là filename)
    pub id: String,
    /// Nguồn (vscode-copilot, cursor, cline, etc.)
    pub source: String,
    /// Tiêu đề (nếu có thể extract nhanh)
    pub title: Option<String>,
    /// Thời gian tạo (nếu có thể extract nhanh)
    pub created_at: Option<DateTime<Utc>>,
    /// Path tới raw file trong vault
    pub vault_path: PathBuf,
    /// Path gốc của file (để debug)
    pub original_path: PathBuf,
    /// Kích thước file (bytes)
    pub file_size: u64,
    /// Workspace name (project name)
    pub workspace_name: Option<String>,
}

/// Thông tin về một session file cần copy
#[derive(Debug, Clone)]
pub struct SessionFile {
    /// Path tới file gốc
    pub source_path: PathBuf,
    /// Metadata cơ bản
    pub metadata: SessionMetadata,
}

/// Trait cho tất cả extractors
/// Extractors chỉ tìm và copy files, KHÔNG parse chi tiết nội dung
pub trait Extractor: Sync {
    /// Tên nguồn (vscode-copilot, cursor, etc.)
    fn source_name(&self) -> &'static str;

    /// Tìm tất cả thư mục chứa chat sessions
    fn find_storage_locations(&self) -> Result<Vec<PathBuf>>;

    /// Lấy tên workspace từ location path
    fn get_workspace_name(&self, location: &Path) -> String;

    /// Liệt kê tất cả session files trong một location
    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>>;

    /// Đếm số sessions trong một location (nhanh, không parse metadata)
    fn count_sessions(&self, location: &Path) -> Result<usize>;

    /// Copy một session file vào vault (incremental - chỉ copy nếu mới/thay đổi)
    /// Trả về Some(path) nếu file được copy, None nếu file không thay đổi (skipped)
    fn copy_to_vault(&self, session: &SessionFile, vault_dir: &Path) -> Result<Option<PathBuf>> {
        // Tạo thư mục con theo source
        let source_dir = vault_dir.join(self.source_name());
        std::fs::create_dir_all(&source_dir)?;

        // Giữ nguyên filename gốc
        let filename = session
            .source_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let dest_path = source_dir.join(&filename);

        // Incremental: chỉ copy nếu file mới hoặc đã thay đổi
        let should_copy = if dest_path.exists() {
            // So sánh kích thước và modified time
            let src_meta = session.source_path.metadata()?;
            let dest_meta = dest_path.metadata()?;

            let src_modified = src_meta.modified()?;
            let dest_modified = dest_meta.modified()?;

            // Copy nếu source mới hơn hoặc kích thước khác
            src_modified > dest_modified || src_meta.len() != dest_meta.len()
        } else {
            true // File chưa tồn tại, cần copy
        };

        if should_copy {
            std::fs::copy(&session.source_path, &dest_path)?;
            Ok(Some(dest_path))
        } else {
            Ok(None) // File không thay đổi
        }
    }
}
