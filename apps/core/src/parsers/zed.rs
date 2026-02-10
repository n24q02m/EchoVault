//! Zed Editor AI Assistant Parser
//!
//! Parses both storage formats from the Zed editor:
//!
//! ## 1. Agent Threads (threads.db - SQLite + zstd)
//! - Schema: `threads(id TEXT PK, parent_id TEXT, summary TEXT, updated_at TEXT, data_type TEXT, data BLOB)`
//! - `data_type` = "json" (raw) or "zstd" (compressed)
//! - Data structure (after decompression):
//!   ```json
//!   {
//!     "version": "0.3.0",
//!     "title": "...",
//!     "messages": [
//!       {"User": {"id": "...", "content": [{"Text": "..."}]}},
//!       {"Agent": {"content": [{"Text": "..."}, {"Thinking": {"text": "...", "signature": "..."}}, {"ToolUse": {...}}], "tool_results": {...}}}
//!     ],
//!     "model": {"provider": "...", "model": "..."},
//!     "updated_at": "RFC3339",
//!     "cumulative_token_usage": {"input_tokens": N, "output_tokens": N},
//!     "detailed_summary": "...",
//!     "profile": "...",
//!     "imported": false,
//!     "subagent_context": null
//!   }
//!   ```
//! - Source: verified from `zed-industries/zed/crates/agent/src/db.rs`
//!
//! ## 2. Text Threads (*.zed.json - legacy JSON files)
//! - Structure:
//!   ```json
//!   {
//!     "version": "0.4.0",
//!     "summary": "...",
//!     "text": "full conversation text buffer",
//!     "messages": [{"id": N, "start": offset, "metadata": {"role": "User|Assistant|System"}}]
//!   }
//!   ```
//!   Messages reference offsets into the `text` buffer.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::Path;

/// Zed Editor Parser
pub struct ZedParser;

impl ZedParser {
    /// Parse Agent Threads from a threads.db SQLite database.
    /// Returns one ParsedConversation per thread.
    fn parse_threads_db(raw_path: &Path) -> Result<Vec<ParsedConversation>> {
        let db = rusqlite::Connection::open_with_flags(
            raw_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Cannot open Zed threads.db")?;

        let mut stmt = db
            .prepare("SELECT id, summary, updated_at, data_type, data FROM threads ORDER BY updated_at DESC")
            .context("Failed to prepare threads query")?;

        let mut conversations = Vec::new();

        let rows: Vec<(String, String, String, String, Vec<u8>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                ))
            })
            .context("Failed to query threads")?
            .filter_map(|r| r.ok())
            .collect();

        for (id, summary, updated_at, data_type, data) in rows {
            // Decompress data if needed
            let json_bytes = if data_type == "zstd" {
                match zstd::decode_all(data.as_slice()) {
                    Ok(decompressed) => decompressed,
                    Err(_) => continue, // Skip corrupt threads
                }
            } else {
                data
            };

            let json_str = match std::str::from_utf8(&json_bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let thread: Value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Extract model info
            let model = thread.get("model").and_then(|m| {
                m.get("model")
                    .or_else(|| m.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            });

            // Parse updated_at
            let updated = DateTime::parse_from_rfc3339(&updated_at)
                .ok()
                .map(|dt| dt.with_timezone(&Utc));

            // Parse messages
            let raw_messages = thread
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let mut messages = Vec::new();
            for msg in &raw_messages {
                let (role, content) = Self::parse_agent_message(msg);
                if content.trim().is_empty() {
                    continue;
                }
                messages.push(ParsedMessage {
                    role,
                    content,
                    timestamp: None,
                    tool_name: None,
                    model: None,
                });
            }

            if messages.is_empty() {
                continue;
            }

            let title = if summary.is_empty() {
                // Fallback: use thread title from JSON or first user message
                thread
                    .get("title")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        messages.iter().find(|m| m.role == Role::User).map(|m| {
                            let first_line = m.content.lines().next().unwrap_or(&m.content);
                            let truncated: String = first_line.chars().take(80).collect();
                            if first_line.chars().count() > 80 {
                                format!("{}...", truncated)
                            } else {
                                truncated
                            }
                        })
                    })
            } else {
                Some(summary)
            };

            conversations.push(ParsedConversation {
                id: format!("zed-agent-{}", id),
                source: "zed".to_string(),
                title,
                workspace: Some("Zed Agent".to_string()),
                created_at: updated,
                updated_at: updated,
                model,
                messages,
                tags: Vec::new(),
            });
        }

        Ok(conversations)
    }

    /// Parse a single message from Agent Thread JSON.
    ///
    /// Zed uses Rust-style tagged enum serialization for messages:
    /// - User: `{"User": {"id": "...", "content": [{"Text": "..."}, ...]}}`
    /// - Agent: `{"Agent": {"content": [{"Text": "..."}, {"Thinking": {...}}, {"ToolUse": {...}}], "tool_results": {...}}}`
    ///
    /// Legacy format (pre-0.3.0) uses: `{"role": "user", "content": "...", "segments": [...]}`
    fn parse_agent_message(msg: &Value) -> (Role, String) {
        // Format 1 (current v0.3.0+): Tagged enum - {"User": {...}} or {"Agent": {...}}
        if let Some(user_data) = msg.get("User") {
            let content = Self::extract_content_parts(
                user_data.get("content").and_then(|c| c.as_array()),
                true,
            );
            return (Role::User, content);
        }

        if let Some(agent_data) = msg.get("Agent") {
            let content = Self::extract_content_parts(
                agent_data.get("content").and_then(|c| c.as_array()),
                false,
            );
            return (Role::Assistant, content);
        }

        // Format 2 (legacy): {"role": "user", "content": "...", "segments": [...]}
        let role_str = msg
            .get("role")
            .and_then(|r| r.as_str())
            .or_else(|| {
                msg.get("metadata")
                    .and_then(|m| m.get("role"))
                    .and_then(|r| r.as_str())
            })
            .unwrap_or("unknown");

        let role = match role_str.to_lowercase().as_str() {
            "user" | "human" => Role::User,
            "assistant" | "model" => Role::Assistant,
            "system" => Role::System,
            "tool" => Role::Tool,
            _ => Role::Info,
        };

        // Legacy: try content as string or structured array
        let content = msg
            .get("content")
            .and_then(|c| {
                if let Some(s) = c.as_str() {
                    Some(s.to_string())
                } else if let Some(arr) = c.as_array() {
                    let parts: Vec<String> = arr
                        .iter()
                        .filter_map(|part| {
                            let part_type = part.get("type").and_then(|t| t.as_str())?;
                            match part_type {
                                "text" => part
                                    .get("text")
                                    .and_then(|t| t.as_str())
                                    .map(|s| s.to_string()),
                                "tool_use" | "tool_call" => {
                                    let name =
                                        part.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                                    Some(format!("[Tool: {}]", name))
                                }
                                "tool_result" => part
                                    .get("content")
                                    .and_then(|c| c.as_str())
                                    .map(|s| s.to_string()),
                                _ => None,
                            }
                        })
                        .collect();
                    if parts.is_empty() {
                        None
                    } else {
                        Some(parts.join("\n"))
                    }
                } else {
                    None
                }
            })
            // Legacy: segments array
            .or_else(|| {
                msg.get("segments").and_then(|s| s.as_array()).map(|segs| {
                    segs.iter()
                        .filter_map(|seg| {
                            if let Some(text_obj) = seg.get("Text") {
                                text_obj
                                    .get("text")
                                    .and_then(|t| t.as_str())
                                    .map(|s| s.to_string())
                            } else {
                                seg.get("text")
                                    .and_then(|t| t.as_str())
                                    .map(|s| s.to_string())
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            })
            .or_else(|| {
                msg.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();

        (role, content)
    }

    /// Extract text from content parts array (tagged enum format).
    ///
    /// User content: `[{"Text": "..."}, ...]`
    /// Agent content: `[{"Text": "..."}, {"Thinking": {"text": "...", "signature": "..."}}, {"ToolUse": {...}}, {"RedactedThinking": "..."}]`
    fn extract_content_parts(parts: Option<&Vec<Value>>, is_user: bool) -> String {
        let Some(parts) = parts else {
            return String::new();
        };

        let mut texts = Vec::new();

        for part in parts {
            // Tagged enum: {"Text": "content"} or {"Text": {"text": "content"}}
            if let Some(text) = part.get("Text") {
                if let Some(s) = text.as_str() {
                    texts.push(s.to_string());
                } else if let Some(s) = text.get("text").and_then(|t| t.as_str()) {
                    texts.push(s.to_string());
                }
                continue;
            }

            // User-specific: {"File": {"path": "..."}}, {"Directory": {...}}, etc.
            if is_user {
                if let Some(file) = part.get("File") {
                    if let Some(path) = file.get("path").and_then(|p| p.as_str()) {
                        texts.push(format!("[File: {}]", path));
                    }
                }
                if let Some(dir) = part.get("Directory") {
                    if let Some(path) = dir.get("path").and_then(|p| p.as_str()) {
                        texts.push(format!("[Directory: {}]", path));
                    }
                }
                continue;
            }

            // Agent-specific: Thinking, ToolUse, RedactedThinking
            if let Some(thinking) = part.get("Thinking") {
                if let Some(text) = thinking.get("text").and_then(|t| t.as_str()) {
                    if !text.trim().is_empty() {
                        texts.push(format!("[Thinking: {}]", text));
                    }
                }
                continue;
            }

            if let Some(tool_use) = part.get("ToolUse") {
                let name = tool_use
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("tool");
                texts.push(format!("[Tool: {}]", name));
                continue;
            }

            // Fallback: any "type"/"text" pattern
            if let Some(part_type) = part.get("type").and_then(|t| t.as_str()) {
                match part_type {
                    "text" => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            texts.push(text.to_string());
                        }
                    }
                    "tool_use" | "tool_call" => {
                        let name = part.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                        texts.push(format!("[Tool: {}]", name));
                    }
                    _ => {}
                }
            }
        }

        texts.join("\n")
    }

    /// Parse a legacy Text Thread JSON file.
    /// These files have a `text` buffer and `messages` array with offset-based boundaries.
    fn parse_text_thread(raw_path: &Path) -> Result<ParsedConversation> {
        let content =
            std::fs::read_to_string(raw_path).context("Cannot read Zed text thread file")?;
        let conversation: Value =
            serde_json::from_str(&content).context("Invalid JSON in Zed text thread")?;

        let session_id = raw_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("zed-text-{}", s))
            .unwrap_or_else(|| "unknown".to_string());

        let title = conversation
            .get("summary")
            .or_else(|| conversation.get("title"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // Check if this has a text buffer (new format) or direct content (simple format)
        let text_buffer = conversation
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let raw_messages = conversation
            .get("messages")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let model = conversation.get("model").and_then(Self::extract_model);

        let mut messages = Vec::new();

        if !text_buffer.is_empty() && !raw_messages.is_empty() {
            // Text buffer format: messages define boundaries in the text
            // Sort messages by start offset
            let mut sorted_msgs: Vec<&Value> = raw_messages.iter().collect();
            sorted_msgs.sort_by_key(|m| m.get("start").and_then(|s| s.as_u64()).unwrap_or(0));

            for (i, msg) in sorted_msgs.iter().enumerate() {
                let start = msg.get("start").and_then(|s| s.as_u64()).unwrap_or(0) as usize;

                // End is the start of the next message, or end of text
                let end = if i + 1 < sorted_msgs.len() {
                    sorted_msgs[i + 1]
                        .get("start")
                        .and_then(|s| s.as_u64())
                        .unwrap_or(text_buffer.len() as u64) as usize
                } else {
                    text_buffer.len()
                };

                let msg_content = if start < text_buffer.len() && end <= text_buffer.len() {
                    text_buffer[start..end].trim().to_string()
                } else {
                    continue;
                };

                if msg_content.is_empty() {
                    continue;
                }

                let role_str = msg
                    .get("metadata")
                    .and_then(|m| m.get("role"))
                    .and_then(|r| r.as_str())
                    .or_else(|| msg.get("role").and_then(|r| r.as_str()))
                    .unwrap_or("unknown");

                let role = match role_str.to_lowercase().as_str() {
                    "user" | "human" => Role::User,
                    "assistant" | "model" => Role::Assistant,
                    "system" => Role::System,
                    "tool" => Role::Tool,
                    _ => Role::Info,
                };
                let is_assistant = role == Role::Assistant;

                messages.push(ParsedMessage {
                    role,
                    content: msg_content,
                    timestamp: None,
                    tool_name: None,
                    model: if is_assistant { model.clone() } else { None },
                });
            }
        } else if !raw_messages.is_empty()
            && raw_messages
                .first()
                .is_some_and(|m| m.get("User").is_some() || m.get("Agent").is_some())
        {
            // v0.3.0+ tagged enum format: {"User": {...}} / {"Agent": {...}}
            // This is the Agent Thread data format used in threads.db, but can also
            // appear in standalone JSON exports.
            for msg in &raw_messages {
                let (role, content) = Self::parse_agent_message(msg);
                if content.trim().is_empty() {
                    continue;
                }
                let is_assistant = role == Role::Assistant;
                messages.push(ParsedMessage {
                    role,
                    content,
                    timestamp: None,
                    tool_name: None,
                    model: if is_assistant { model.clone() } else { None },
                });
            }
        } else {
            // Simple format: messages with direct content (role + content string)
            for msg in &raw_messages {
                let role_str = msg
                    .get("role")
                    .and_then(|r| r.as_str())
                    .unwrap_or("unknown");

                let role = match role_str.to_lowercase().as_str() {
                    "user" | "human" => Role::User,
                    "assistant" | "model" => Role::Assistant,
                    "system" => Role::System,
                    "tool" => Role::Tool,
                    _ => Role::Info,
                };

                let msg_content = msg
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or_default()
                    .to_string();

                if msg_content.trim().is_empty() {
                    continue;
                }

                messages.push(ParsedMessage {
                    role,
                    content: msg_content,
                    timestamp: None,
                    tool_name: None,
                    model: if role_str == "assistant" {
                        model.clone()
                    } else {
                        None
                    },
                });
            }
        }

        let created_at = conversation
            .get("updated_at")
            .or_else(|| conversation.get("created_at"))
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .or_else(|| {
                std::fs::metadata(raw_path)
                    .ok()
                    .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .and_then(|d| DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0))
            });

        // Title fallback
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
            source: "zed".to_string(),
            title,
            workspace: Some("Zed".to_string()),
            created_at,
            updated_at: created_at,
            model,
            messages,
            tags: Vec::new(),
        })
    }

    /// Extract model name from the model field.
    /// Can be: a string, or an object like {"provider": "zed", "model": "claude-3-5-sonnet"}
    fn extract_model(model_value: &Value) -> Option<String> {
        match model_value {
            Value::String(s) => Some(s.clone()),
            Value::Object(obj) => obj
                .get("model")
                .or_else(|| obj.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }
    }
}

impl Parser for ZedParser {
    fn source_name(&self) -> &'static str {
        "zed"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let extension = raw_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let filename = raw_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if filename == "threads.db" || extension == "db" {
            // Agent Threads: SQLite database with multiple threads
            let conversations = Self::parse_threads_db(raw_path)?;

            if conversations.is_empty() {
                return Ok(ParsedConversation {
                    id: "zed-agent-empty".to_string(),
                    source: "zed".to_string(),
                    title: None,
                    workspace: Some("Zed Agent".to_string()),
                    created_at: None,
                    updated_at: None,
                    model: None,
                    messages: Vec::new(),
                    tags: Vec::new(),
                });
            }

            if conversations.len() == 1 {
                return Ok(conversations.into_iter().next().unwrap());
            }

            // Multiple threads: merge with separators
            let mut merged_messages = Vec::new();
            let first_created = conversations.first().and_then(|c| c.created_at);
            let last_updated = conversations.last().and_then(|c| c.updated_at);

            for (i, conv) in conversations.iter().enumerate() {
                if i > 0 {
                    let sep_title = conv.title.as_deref().unwrap_or("(untitled)");
                    merged_messages.push(ParsedMessage {
                        role: Role::Info,
                        content: format!("--- Thread: {} ---", sep_title),
                        timestamp: conv.created_at,
                        tool_name: None,
                        model: None,
                    });
                }
                merged_messages.extend(conv.messages.clone());
            }

            Ok(ParsedConversation {
                id: "zed-agent-all".to_string(),
                source: "zed".to_string(),
                title: Some(format!("Zed Agent ({} threads)", conversations.len())),
                workspace: Some("Zed Agent".to_string()),
                created_at: first_created,
                updated_at: last_updated,
                model: None,
                messages: merged_messages,
                tags: Vec::new(),
            })
        } else {
            // Text Threads: legacy JSON format
            Self::parse_text_thread(raw_path)
        }
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        let filename = raw_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        filename == "threads.db"
            || filename.ends_with(".zed.json")
            || (filename.ends_with(".json") && !filename.ends_with("sessions.json"))
    }
}
