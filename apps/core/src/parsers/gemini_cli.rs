//! Gemini CLI Parser
//!
//! Parses session JSON files from Google Gemini CLI.
//!
//! Format: JSON object (ConversationRecord):
//! ```json
//! {
//!   "sessionId": "...",
//!   "projectHash": "...",
//!   "startTime": "2024-01-15T10:30:00Z",
//!   "lastUpdated": "2024-01-15T11:45:00Z",
//!   "summary": "...",
//!   "messages": [
//!     {
//!       "id": "...",
//!       "timestamp": "2024-01-15T10:30:05Z",
//!       "type": "user" | "gemini" | "info" | "error" | "warning",
//!       "content": "..." | [{ "text": "..." }]
//!     }
//!   ]
//! }
//! ```

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::Path;

/// Gemini CLI Parser
pub struct GeminiCliParser;

impl GeminiCliParser {
    /// Extract text content from Gemini message content field.
    /// Content can be a string or an array of parts.
    fn extract_content(content: &Value) -> String {
        match content {
            Value::String(s) => s.clone(),
            Value::Array(arr) => {
                let texts: Vec<String> = arr
                    .iter()
                    .filter_map(|part| part.get("text").and_then(|t| t.as_str()).map(String::from))
                    .collect();
                texts.join("\n\n")
            }
            _ => String::new(),
        }
    }
}

impl Parser for GeminiCliParser {
    fn source_name(&self) -> &'static str {
        "gemini-cli"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let content =
            std::fs::read_to_string(raw_path).context("Cannot read Gemini CLI session file")?;

        let json: Value =
            serde_json::from_str(&content).context("Invalid JSON in Gemini CLI session")?;

        let session_id = json
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                raw_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
            })
            .to_string();

        let title = json
            .get("summary")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);

        let created_at = json
            .get("startTime")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let updated_at = json
            .get("lastUpdated")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let workspace = json
            .get("projectHash")
            .and_then(|v| v.as_str())
            .map(String::from);

        let mut messages = Vec::new();

        if let Some(msg_array) = json.get("messages").and_then(|v| v.as_array()) {
            for msg in msg_array {
                let msg_type = msg
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");

                let role = match msg_type {
                    "user" => Role::User,
                    "gemini" | "model" => Role::Assistant,
                    "info" => Role::Info,
                    "error" | "warning" => Role::Info,
                    "tool_call" | "function_call" => Role::Tool,
                    "tool_result" | "function_response" => Role::Tool,
                    _ => Role::Info,
                };

                let content_text = msg
                    .get("content")
                    .map(Self::extract_content)
                    .unwrap_or_default();

                if content_text.trim().is_empty() {
                    continue;
                }

                // Prefix error/warning messages
                let content_text = match msg_type {
                    "error" => format!("**Error:** {}", content_text),
                    "warning" => format!("**Warning:** {}", content_text),
                    _ => content_text,
                };

                let timestamp = msg
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc));

                let tool_name = if role == Role::Tool {
                    msg.get("name")
                        .or_else(|| msg.get("functionName"))
                        .and_then(|n| n.as_str())
                        .map(String::from)
                } else {
                    None
                };

                messages.push(ParsedMessage {
                    role,
                    content: content_text,
                    timestamp,
                    tool_name,
                    model: None,
                });
            }
        }

        let title = title.or_else(|| {
            messages.iter().find(|m| m.role == Role::User).map(|m| {
                let first_line = m.content.lines().next().unwrap_or(&m.content);
                let truncated: String = first_line.chars().take(80).collect();
                if first_line.chars().count() > 80 {
                    format!("{}...", truncated)
                } else {
                    truncated
                }
            })
        });

        Ok(ParsedConversation {
            id: session_id,
            source: "gemini-cli".to_string(),
            title,
            workspace,
            created_at,
            updated_at,
            model: None,
            messages,
            tags: Vec::new(),
        })
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path.extension().is_some_and(|ext| ext == "json")
    }
}
