//! Aider Parser
//!
//! Parses .aider.chat.history.md files which are already in Markdown format.
//! The parser normalizes the format to match our standard ParsedConversation structure.
//!
//! Aider chat history format:
//! ```markdown
//! # aider chat started at 2024-01-15 10:30:00
//!
//! #### /ask How do I implement this?
//!
//! I would suggest...
//!
//! #### /code Fix the bug in main.rs
//!
//! Here's the fix...
//! ```
//!
//! Lines starting with `####` are user commands/messages.
//! Everything between `####` lines is the assistant response.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use std::path::Path;

/// Aider Parser
pub struct AiderParser;

impl Parser for AiderParser {
    fn source_name(&self) -> &'static str {
        "aider"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let content =
            std::fs::read_to_string(raw_path).context("Cannot read Aider chat history file")?;

        // Session ID from parent directory (project name)
        let project_dir = raw_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let session_id = format!(
            "aider-{}",
            project_dir
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect::<String>()
        );

        let mut messages: Vec<ParsedMessage> = Vec::new();
        let mut created_at: Option<DateTime<Utc>> = None;
        let mut current_response = String::new();
        let mut in_response = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Parse session start timestamp
            // "# aider chat started at 2024-01-15 10:30:00"
            if trimmed.starts_with("# aider chat started at ") {
                let ts_str = trimmed
                    .trim_start_matches("# aider chat started at ")
                    .trim();
                if created_at.is_none() {
                    created_at = NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d %H:%M:%S")
                        .ok()
                        .map(|ndt| ndt.and_utc());
                }
                continue;
            }

            // User commands start with ####
            if trimmed.starts_with("#### ") {
                // Save previous assistant response if any
                if in_response && !current_response.trim().is_empty() {
                    messages.push(ParsedMessage {
                        role: Role::Assistant,
                        content: current_response.trim().to_string(),
                        timestamp: None,
                        tool_name: None,
                        model: None,
                    });
                    current_response.clear();
                }

                // Extract user command
                let command = trimmed.trim_start_matches("#### ").trim();
                if !command.is_empty() {
                    messages.push(ParsedMessage {
                        role: Role::User,
                        content: command.to_string(),
                        timestamp: None,
                        tool_name: None,
                        model: None,
                    });
                }

                in_response = true;
                continue;
            }

            // Skip separator lines
            if trimmed == "---" || trimmed.is_empty() && !in_response {
                continue;
            }

            // Accumulate assistant response
            if in_response {
                current_response.push_str(line);
                current_response.push('\n');
            }
        }

        // Don't forget the last response
        if in_response && !current_response.trim().is_empty() {
            messages.push(ParsedMessage {
                role: Role::Assistant,
                content: current_response.trim().to_string(),
                timestamp: None,
                tool_name: None,
                model: None,
            });
        }

        // Title from first user command
        let title = messages.iter().find(|m| m.role == Role::User).map(|m| {
            let command = &m.content;
            // Strip aider commands like /ask, /code, /architect etc.
            let clean = if command.starts_with('/') {
                command
                    .split_once(' ')
                    .map(|(_, rest)| rest)
                    .unwrap_or(command)
            } else {
                command
            };
            let truncated: String = clean.chars().take(80).collect();
            if clean.chars().count() > 80 {
                format!("{}...", truncated)
            } else {
                truncated
            }
        });

        // Fallback timestamp from file metadata
        if created_at.is_none() {
            created_at = std::fs::metadata(raw_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));
        }

        Ok(ParsedConversation {
            id: session_id,
            source: "aider".to_string(),
            title,
            workspace: Some(project_dir.to_string()),
            created_at,
            updated_at: None,
            model: None,
            messages,
            tags: Vec::new(),
        })
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path.file_name().is_some_and(|n| {
            let name = n.to_string_lossy();
            name == ".aider.chat.history.md" || name.ends_with(".aider.chat.history.md")
        })
    }
}
