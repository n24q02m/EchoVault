//! Markdown Formatter
//!
//! Chuyển đổi ChatSession thành Markdown với frontmatter metadata.

use super::Formatter;
use crate::extractors::ChatSession;
use anyhow::Result;
use chrono::Utc;

/// Markdown formatter với frontmatter
pub struct MarkdownFormatter;

impl MarkdownFormatter {
    pub fn new() -> Self {
        Self
    }

    /// Escape các ký tự đặc biệt trong title cho frontmatter
    fn escape_title(title: &str) -> String {
        title
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', " ")
            .replace('\r', "")
    }

    /// Tạo slug từ title
    fn slugify(title: &str) -> String {
        title
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }
}

impl Default for MarkdownFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for MarkdownFormatter {
    fn format(&self, session: &ChatSession, workspace_name: &str) -> Result<String> {
        let mut output = String::new();

        // Frontmatter
        output.push_str("---\n");

        // Title
        let title = session
            .title
            .as_ref()
            .map(|t| Self::escape_title(t))
            .unwrap_or_else(|| "Untitled Session".to_string());
        output.push_str(&format!("title: \"{}\"\n", title));

        // Date
        let date = session
            .created_at
            .unwrap_or_else(Utc::now)
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        output.push_str(&format!("date: {}\n", date));

        // Source
        output.push_str(&format!("source: {}\n", session.source));

        // Project
        output.push_str(&format!("project: {}\n", workspace_name));

        // Session ID
        output.push_str(&format!("session_id: {}\n", session.id));

        output.push_str("---\n\n");

        // Messages
        for (idx, message) in session.messages.iter().enumerate() {
            // Role header
            let role_display = match message.role.as_str() {
                "user" => "User",
                "assistant" => {
                    if let Some(model) = &message.model {
                        &format!("Assistant ({})", model)
                    } else {
                        "Assistant"
                    }
                }
                _ => &message.role,
            };

            output.push_str(&format!("## {}\n\n", role_display));

            // Content
            output.push_str(&message.content);
            output.push_str("\n\n");

            // Separator giữa các cặp Q&A (trừ message cuối)
            if idx < session.messages.len() - 1 && message.role == "assistant" {
                output.push_str("---\n\n");
            }
        }

        Ok(output)
    }

    fn generate_filename(&self, session: &ChatSession) -> String {
        // Format: YYYY-MM-DD_HH-MM-SS_slug_id.md
        // Thêm session_id (8 ký tự đầu) để tránh trùng filename
        let date = session
            .created_at
            .unwrap_or_else(Utc::now)
            .format("%Y-%m-%d_%H-%M-%S")
            .to_string();

        let slug = session
            .title
            .as_ref()
            .map(|t| Self::slugify(t))
            .unwrap_or_else(|| "untitled".to_string());

        // Giới hạn độ dài slug (at char boundary, not byte boundary)
        let slug: String = slug.chars().take(40).collect();

        // Lấy 8 ký tự đầu của session_id để đảm bảo unique
        let id_suffix: String = session.id.chars().take(8).collect();

        format!("{}_{}_{}.md", date, slug, id_suffix)
    }
}
