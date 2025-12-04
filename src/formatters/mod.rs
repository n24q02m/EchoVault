//! Formatters module - Chuyển đổi chat sessions thành các định dạng output.

pub mod markdown;

use crate::extractors::ChatSession;
use anyhow::Result;

/// Trait cho tất cả formatters
pub trait Formatter {
    /// Format một session thành string
    fn format(&self, session: &ChatSession, workspace_name: &str) -> Result<String>;

    /// Tạo tên file cho session
    fn generate_filename(&self, session: &ChatSession) -> String;
}
