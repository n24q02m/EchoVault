//! Antigravity Parser
//!
//! Handles two types of Antigravity files:
//! 1. Conversation .pb files (binary protobuf) - These cannot be fully parsed without
//!    the protobuf schema. We attempt best-effort text extraction from the binary.
//! 2. Brain artifacts (.md files) - Already in Markdown, just normalize.
//!
//! For .pb files, we use a heuristic approach to extract UTF-8 text strings
//! from the binary data without requiring the protobuf schema definition.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::Path;

/// Antigravity Parser
pub struct AntigravityParser;

impl AntigravityParser {
    /// Best-effort extraction of UTF-8 text strings from protobuf binary data.
    /// Protobuf string fields are encoded as: field_tag (varint) + length (varint) + UTF-8 bytes.
    /// We look for sequences of printable UTF-8 text longer than a threshold.
    fn extract_text_from_pb(data: &[u8]) -> Vec<String> {
        let mut texts = Vec::new();
        let mut current = String::new();
        let min_length = 20; // Minimum meaningful text length

        let mut i = 0;
        while i < data.len() {
            // Try to decode as UTF-8 character
            let remaining = &data[i..];
            match std::str::from_utf8(remaining.get(..1).unwrap_or_default()) {
                Ok(ch) => {
                    let c = ch.chars().next().unwrap_or('\0');
                    if c.is_ascii_graphic() || c.is_ascii_whitespace() {
                        current.push(c);
                    } else {
                        if current.len() >= min_length {
                            let trimmed = current.trim().to_string();
                            if !trimmed.is_empty()
                                && trimmed.chars().filter(|c| c.is_alphabetic()).count() > 5
                            {
                                texts.push(trimmed);
                            }
                        }
                        current.clear();
                    }
                    i += 1;
                }
                Err(_) => {
                    // Multi-byte UTF-8 or invalid
                    if current.len() >= min_length {
                        let trimmed = current.trim().to_string();
                        if !trimmed.is_empty()
                            && trimmed.chars().filter(|c| c.is_alphabetic()).count() > 5
                        {
                            texts.push(trimmed);
                        }
                    }
                    current.clear();
                    i += 1;
                }
            }
        }

        // Don't forget the last segment
        if current.len() >= min_length {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() && trimmed.chars().filter(|c| c.is_alphabetic()).count() > 5 {
                texts.push(trimmed);
            }
        }

        texts
    }

    /// Parse a .pb conversation file with best-effort text extraction.
    fn parse_pb(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let data = std::fs::read(raw_path).context("Cannot read Antigravity .pb file")?;

        let session_id = raw_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let texts = Self::extract_text_from_pb(&data);

        let mut messages = Vec::new();

        // Heuristic: alternate between user and assistant messages.
        // The first long text is often a system/context message, subsequent ones
        // alternate between user queries and assistant responses.
        let mut is_user = true;
        for text in &texts {
            // Skip very short or likely metadata strings
            if text.len() < 30 {
                continue;
            }

            let role = if is_user { Role::User } else { Role::Assistant };
            messages.push(ParsedMessage {
                role,
                content: text.clone(),
                timestamp: None,
                tool_name: None,
                model: None,
            });
            is_user = !is_user;
        }

        // If we couldn't extract meaningful messages, create a placeholder
        if messages.is_empty() {
            messages.push(ParsedMessage {
                role: Role::Info,
                content: format!(
                    "*Binary protobuf conversation ({} bytes). Text extraction yielded no readable content. \
                     Use the Antigravity proxy interceptor to capture future conversations in readable format.*",
                    data.len()
                ),
                timestamp: None,
                tool_name: None,
                model: None,
            });
        }

        let created_at = std::fs::metadata(raw_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));

        let title = messages
            .iter()
            .find(|m| m.role == Role::User)
            .map(|m| {
                let first_line = m.content.lines().next().unwrap_or(&m.content);
                let truncated: String = first_line.chars().take(80).collect();
                if first_line.chars().count() > 80 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            })
            .or_else(|| Some("Antigravity Conversation".to_string()));

        Ok(ParsedConversation {
            id: session_id,
            source: "antigravity".to_string(),
            title,
            workspace: None,
            created_at,
            updated_at: None,
            model: None,
            messages,
            tags: Vec::new(),
        })
    }

    /// Parse a brain artifact .md file.
    fn parse_artifact(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let content =
            std::fs::read_to_string(raw_path).context("Cannot read Antigravity artifact file")?;

        let filename = raw_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("artifact");

        // Session ID includes parent UUID dir
        let session_id = raw_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|parent| format!("{}_{}", parent, filename))
            .unwrap_or_else(|| filename.to_string());

        let created_at = std::fs::metadata(raw_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0));

        // Try to read companion metadata JSON
        let metadata_path = raw_path.with_extension("md.metadata.json");
        let (meta_title, meta_timestamp) = if metadata_path.exists() {
            std::fs::read_to_string(&metadata_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .map(|json| {
                    let title = json
                        .get("summary")
                        .or_else(|| json.get("artifactType"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let ts = json
                        .get("updatedAt")
                        .and_then(|v| v.as_str())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc));
                    (title, ts)
                })
                .unwrap_or((None, None))
        } else {
            (None, None)
        };

        let title = meta_title.or_else(|| Some(filename.replace('_', " ")));

        Ok(ParsedConversation {
            id: session_id,
            source: "antigravity-artifact".to_string(),
            title,
            workspace: None,
            created_at: meta_timestamp.or(created_at),
            updated_at: None,
            model: None,
            messages: vec![ParsedMessage {
                role: Role::Assistant,
                content,
                timestamp: meta_timestamp.or(created_at),
                tool_name: None,
                model: None,
            }],
            tags: vec!["artifact".to_string()],
        })
    }
}

impl Parser for AntigravityParser {
    fn source_name(&self) -> &'static str {
        "antigravity"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let ext = raw_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "pb" => self.parse_pb(raw_path),
            "md" => self.parse_artifact(raw_path),
            _ => anyhow::bail!("Unsupported Antigravity file type: {}", raw_path.display()),
        }
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path
            .extension()
            .is_some_and(|ext| ext == "pb" || ext == "md")
    }
}
