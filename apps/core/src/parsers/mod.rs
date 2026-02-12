//! Parsers module - Parse raw extracted files into clean structured conversations.
//!
//! Phase 2 of the EchoVault pipeline:
//! 1. Extractors copy raw files to vault (Phase 1)
//! 2. Parsers read raw files and produce ParsedConversation (Phase 2)
//! 3. Markdown writer serializes to clean .md with YAML frontmatter
//!
//! Each parser knows how to read its source format and produce
//! a unified ParsedConversation structure.

pub mod aider;
pub mod antigravity;
pub mod claude_code;
pub mod cline;
pub mod codex;
pub mod continue_dev;
pub mod cursor;
pub mod gemini_cli;
pub mod jetbrains;
pub mod markdown_writer;
pub mod opencode;
pub mod vscode_copilot;
pub mod zed;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Role of a message sender.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Human user
    User,
    /// AI assistant
    Assistant,
    /// System prompt or context
    System,
    /// Tool call or result
    Tool,
    /// Informational message (e.g., status, error)
    Info,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System => write!(f, "system"),
            Role::Tool => write!(f, "tool"),
            Role::Info => write!(f, "info"),
        }
    }
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedMessage {
    /// Role of the message sender
    pub role: Role,
    /// Message content (plain text or Markdown)
    pub content: String,
    /// Timestamp of this message (if available)
    pub timestamp: Option<DateTime<Utc>>,
    /// Tool name (if role == Tool)
    pub tool_name: Option<String>,
    /// Model used for this response (if available, for assistant messages)
    pub model: Option<String>,
}

/// A fully parsed conversation with all messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedConversation {
    /// Unique session ID
    pub id: String,
    /// Source identifier (vscode-copilot, cursor, cline, etc.)
    pub source: String,
    /// Conversation title
    pub title: Option<String>,
    /// Workspace/project name
    pub workspace: Option<String>,
    /// When the conversation started
    pub created_at: Option<DateTime<Utc>>,
    /// When the conversation was last updated
    pub updated_at: Option<DateTime<Utc>>,
    /// Model used (if consistent across conversation)
    pub model: Option<String>,
    /// All messages in chronological order
    pub messages: Vec<ParsedMessage>,
    /// Tags for categorization (auto-extracted or user-defined)
    pub tags: Vec<String>,
}

impl ParsedConversation {
    /// Count messages by role.
    pub fn count_by_role(&self, role: &Role) -> usize {
        self.messages.iter().filter(|m| &m.role == role).count()
    }

    /// Get total content length in characters.
    pub fn total_content_len(&self) -> usize {
        self.messages.iter().map(|m| m.content.len()).sum()
    }

    /// Check if conversation is empty (no real messages).
    pub fn is_empty(&self) -> bool {
        self.messages
            .iter()
            .all(|m| m.role == Role::System || m.role == Role::Info || m.content.trim().is_empty())
    }
}

/// Trait for all parsers.
/// A parser reads a raw file from the vault and produces a ParsedConversation.
pub trait Parser: Sync {
    /// Source name this parser handles (must match extractor source_name)
    fn source_name(&self) -> &'static str;

    /// Parse a single raw file into a ParsedConversation.
    /// `raw_path` is the path to the raw file in the vault.
    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation>;

    /// Check if this parser can handle the given file.
    fn can_parse(&self, raw_path: &Path) -> bool;
}

/// Parse all raw files in a vault directory for a given source.
/// Returns (successful_parses, errors).
pub fn parse_vault_source(
    parser: &dyn Parser,
    vault_dir: &Path,
) -> (Vec<ParsedConversation>, Vec<(PathBuf, anyhow::Error)>) {
    let source_dir = vault_dir.join(parser.source_name());
    let mut conversations = Vec::new();
    let mut errors = Vec::new();

    if !source_dir.exists() {
        return (conversations, errors);
    }

    // Walk the source directory for parseable files
    let mut files = Vec::new();
    collect_files_recursive(&source_dir, &mut files);

    for file_path in files {
        if !parser.can_parse(&file_path) {
            continue;
        }

        match parser.parse(&file_path) {
            Ok(conv) => {
                if !conv.is_empty() {
                    conversations.push(conv);
                }
            }
            Err(e) => {
                errors.push((file_path, e));
            }
        }
    }

    // Sort by created_at (newest first)
    conversations.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    (conversations, errors)
}

/// Recursively collect all files in a directory.
fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursive(&path, files);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
}

/// Create all parsers.
pub fn all_parsers() -> Vec<Box<dyn Parser>> {
    vec![
        Box::new(vscode_copilot::VSCodeCopilotParser),
        Box::new(cursor::CursorParser),
        Box::new(cline::ClineParser),
        Box::new(gemini_cli::GeminiCliParser),
        Box::new(claude_code::ClaudeCodeParser),
        Box::new(codex::CodexParser),
        Box::new(aider::AiderParser),
        Box::new(antigravity::AntigravityParser),
        Box::new(continue_dev::ContinueDevParser),
        Box::new(opencode::OpenCodeParser),
        Box::new(zed::ZedParser),
        Box::new(jetbrains::JetBrainsParser),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;

    #[test]
    fn test_collect_files_recursive_deep() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create deep structure: depth 5, 5 files per folder
        create_files(root, 5, 5);

        // Measure time
        let start = std::time::Instant::now();
        let mut files = Vec::new();
        collect_files_recursive(root, &mut files);
        let duration = start.elapsed();

        println!("Time taken: {:?}", duration);
        // depth 5 implies levels 0 to 5, i.e., 6 levels.
        // files per level = 5.
        // Total = 6 * 5 = 30.
        assert_eq!(files.len(), 30);
    }

    fn create_files(dir: &Path, depth: usize, count: usize) {
        fs::create_dir_all(dir).unwrap();
        for i in 0..count {
            File::create(dir.join(format!("file_{}.txt", i))).unwrap();
        }
        if depth > 0 {
            create_files(&dir.join("sub"), depth - 1, count);
        }
    }
}
