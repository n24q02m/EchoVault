//! Extractors module - Trích xuất chat history từ các IDE khác nhau.

pub mod vscode_copilot;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Một message trong chat session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Vai trò: "user" hoặc "assistant"
    pub role: String,
    /// Nội dung message
    pub content: String,
    /// Tên model (nếu có)
    pub model: Option<String>,
}

/// Một chat session hoàn chỉnh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    /// ID duy nhất của session
    pub id: String,
    /// Tiêu đề (thường là message đầu tiên)
    pub title: Option<String>,
    /// Nguồn (vscode-copilot, cursor, cline, etc.)
    pub source: String,
    /// Thời gian tạo
    pub created_at: Option<DateTime<Utc>>,
    /// Thời gian cập nhật cuối
    pub updated_at: Option<DateTime<Utc>>,
    /// Danh sách messages
    pub messages: Vec<ChatMessage>,
}

/// Trait cho tất cả extractors
pub trait Extractor {
    /// Tìm tất cả database files
    fn find_databases(&self) -> Result<Vec<PathBuf>>;

    /// Đếm số sessions trong một database
    fn count_sessions(&self, db_path: &PathBuf) -> Result<usize>;

    /// Trích xuất tất cả sessions từ một database
    fn extract_sessions(&self, db_path: &PathBuf) -> Result<Vec<ChatSession>>;
}
