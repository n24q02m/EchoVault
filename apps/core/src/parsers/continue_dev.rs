//! Continue.dev Parser
//!
//! Parses JSON session files from the Continue.dev extension.
//!
//! Format: Single JSON file per session with structure:
//! ```json
//! {
//!   "sessionId": "uuid",
//!   "title": "Session Title",
//!   "workspaceDirectory": "/path/to/project",
//!   "history": [
//!     {
//!       "message": {
//!         "role": "user",
//!         "content": "string or MessagePart[]"
//!       },
//!       "contextItems": [...]
//!     }
//!   ],
//!   "mode": "chat",
//!   "chatModelTitle": "gpt-4"
//! }
//! ```
//!
//! Message roles: "user", "assistant", "system", "thinking", "tool"
//! Content can be a plain string or array of {type: "text", text: "..."} parts.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

/// Continue.dev Parser
pub struct ContinueDevParser;

impl ContinueDevParser {
    /// Extract text content from a ChatMessage content field.
    /// Content can be:
    /// - A string: "hello"
    /// - An array of parts: [{"type": "text", "text": "hello"}, {"type": "imageUrl", ...}]
    fn extract_content(content: &Value) -> String {
        match content {
            Value::String(s) => s.clone(),
            Value::Array(parts) => {
                let mut text = String::new();
                for part in parts {
                    if let Some(part_type) = part.get("type").and_then(|t| t.as_str()) {
                        match part_type {
                            "text" => {
                                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                                    if !text.is_empty() {
                                        text.push('\n');
                                    }
                                    text.push_str(t);
                                }
                            }
                            "imageUrl" => {
                                text.push_str("[image]");
                            }
                            _ => {}
                        }
                    }
                }
                text
            }
            _ => String::new(),
        }
    }
}

impl Parser for ContinueDevParser {
    fn source_name(&self) -> &'static str {
        "continue-dev"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let content =
            std::fs::read_to_string(raw_path).context("Cannot read Continue.dev session file")?;
        let session: Value =
            serde_json::from_str(&content).context("Invalid JSON in Continue.dev session")?;

        let session_id = session
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let title = session
            .get("title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let workspace = session
            .get("workspaceDirectory")
            .and_then(|v| v.as_str())
            .and_then(|ws| {
                std::path::PathBuf::from(ws)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            });

        let model = session
            .get("chatModelTitle")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse dateCreated from session JSON (ISO date string)
        let date_created = session
            .get("dateCreated")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .or_else(|| {
                        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                            .ok()
                            .map(|ndt| ndt.and_utc())
                    })
            });

        // Extract usage stats if present
        let usage_tag = session.get("usage").and_then(|u| {
            let cost = u.get("totalCost").and_then(|c| c.as_f64());
            let prompt = u.get("promptTokens").and_then(|p| p.as_u64());
            let completion = u.get("completionTokens").and_then(|c| c.as_u64());
            match (cost, prompt, completion) {
                (Some(c), Some(p), Some(comp)) => {
                    Some(format!("cost:{:.4} prompt:{} completion:{}", c, p, comp))
                }
                (Some(c), _, _) => Some(format!("cost:{:.4}", c)),
                _ => None,
            }
        });

        let history = session
            .get("history")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut messages = Vec::new();

        for item in &history {
            let message = match item.get("message") {
                Some(msg) => msg,
                None => continue,
            };

            let role_str = message
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("unknown");

            let role = match role_str {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                "thinking" => Role::System, // Map thinking to system
                "tool" => Role::Tool,
                _ => Role::Info,
            };

            // Extract content (string or array of parts)
            let content = message
                .get("content")
                .map(Self::extract_content)
                .unwrap_or_default();

            // For thinking messages, prefix with marker
            let content = if role_str == "thinking" && !content.is_empty() {
                format!("**[Thinking]** {}", content)
            } else {
                content
            };

            if content.trim().is_empty() {
                // Check for tool calls in assistant messages
                if let Some(tool_calls) = message.get("toolCalls").and_then(|tc| tc.as_array()) {
                    for tc in tool_calls {
                        let func = tc.get("function").unwrap_or(tc);
                        let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                        let args = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("");

                        let desc = if args.len() > 200 {
                            format!("Called `{}` with args (truncated)", name)
                        } else if !args.is_empty() {
                            format!("Called `{}`: {}", name, args)
                        } else {
                            format!("Called `{}`", name)
                        };

                        messages.push(ParsedMessage {
                            role: Role::Tool,
                            content: desc,
                            timestamp: None,
                            tool_name: Some(name.to_string()),
                            model: None,
                        });
                    }
                }
                continue;
            }

            messages.push(ParsedMessage {
                role,
                content,
                timestamp: None,
                tool_name: if role_str == "tool" {
                    message
                        .get("toolCallId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                },
                model: if role_str == "assistant" {
                    model.clone()
                } else {
                    None
                },
            });
        }

        // Title fallback: first user message
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

        // Timestamp from JSON dateCreated, then fallback to file metadata
        let created_at = date_created.or_else(|| {
            std::fs::metadata(raw_path)
                .ok()
                .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|d| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                })
        });

        let mut tags = Vec::new();
        if let Some(mode) = session.get("mode").and_then(|v| v.as_str()) {
            if mode != "chat" {
                tags.push(format!("mode:{}", mode));
            }
        }
        if let Some(usage) = usage_tag {
            tags.push(usage);
        }

        Ok(ParsedConversation {
            id: session_id,
            source: "continue-dev".to_string(),
            title,
            workspace,
            created_at,
            updated_at: created_at,
            model,
            messages,
            tags,
        })
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path.extension().is_some_and(|ext| ext == "json")
            && raw_path
                .file_name()
                .is_some_and(|name| name != "sessions.json")
    }
}
