//! OpenCode Parser
//!
//! Parses SQLite database files from OpenCode (github.com/opencode-ai/opencode).
//!
//! Database schema:
//! - sessions: id TEXT PK, title TEXT, message_count INTEGER,
//!   prompt_tokens INTEGER, completion_tokens INTEGER,
//!   cost REAL, created_at INTEGER, updated_at INTEGER
//! - messages: id TEXT PK, session_id TEXT FK, role TEXT, parts TEXT (JSON array),
//!   model TEXT, created_at INTEGER, updated_at INTEGER
//!
//! Parts JSON: `[{"type":"text","text":"..."}, {"type":"tool_call","name":"...","input":{...}}, ...]`
//!
//! Message roles: "user", "assistant"
//! The database stores timestamps as Unix epoch seconds.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::Path;

/// OpenCode Parser
pub struct OpenCodeParser;

impl OpenCodeParser {
    /// Extract text content from the `parts` JSON array.
    ///
    /// Parts format: `[{"type":"text","text":"..."}, {"type":"tool_call","name":"edit","input":{...}}, ...]`
    fn extract_content_from_parts(parts_json: &str) -> String {
        let parts: Vec<serde_json::Value> = match serde_json::from_str(parts_json) {
            Ok(v) => v,
            Err(_) => {
                // Fallback: treat raw string as content
                return if parts_json.is_empty() {
                    String::new()
                } else {
                    parts_json.to_string()
                };
            }
        };

        let mut text_parts = Vec::new();

        for part in &parts {
            let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match part_type {
                "text" => {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            text_parts.push(text.to_string());
                        }
                    }
                }
                "reasoning" => {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            text_parts.push(format!("**[Reasoning]** {}", text));
                        }
                    }
                }
                "tool_call" => {
                    let name = part.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                    text_parts.push(format!("[Tool: {}]", name));
                }
                "tool_result" => {
                    if let Some(content) = part.get("content").and_then(|c| c.as_str()) {
                        let truncated: String = content.chars().take(500).collect();
                        if content.chars().count() > 500 {
                            text_parts.push(format!("[Result] {}...", truncated));
                        } else {
                            text_parts.push(format!("[Result] {}", truncated));
                        }
                    }
                }
                "finish" => {
                    // Skip finish markers
                }
                _ => {}
            }
        }

        text_parts.join("\n")
    }

    /// Extract tool name from parts JSON for tool-related messages.
    fn extract_tool_name_from_parts(parts_json: &str) -> Option<String> {
        let parts: Vec<serde_json::Value> = serde_json::from_str(parts_json).ok()?;

        for part in &parts {
            let part_type = part.get("type").and_then(|t| t.as_str())?;
            if part_type == "tool_call" {
                return part
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
            }
        }

        None
    }
}

impl Parser for OpenCodeParser {
    fn source_name(&self) -> &'static str {
        "opencode"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        // Open the SQLite database in read-only mode
        let db = rusqlite::Connection::open_with_flags(
            raw_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Cannot open OpenCode database")?;

        // Query all sessions
        let mut all_conversations = Vec::new();

        let mut session_stmt = db
            .prepare(
                "SELECT id, title, model, created_at, updated_at FROM sessions \
                 ORDER BY created_at DESC",
            )
            .context("Failed to prepare sessions query")?;

        type SessionRow = (
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
        );
        let sessions: Vec<SessionRow> = session_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            })
            .context("Failed to query sessions")?
            .filter_map(|r| r.ok())
            .collect();

        for (session_id, title, model, created_ts, updated_ts) in sessions {
            let created_at = created_ts.and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0));
            let updated_at = updated_ts.and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0));

            // Query messages for this session
            // Schema: role TEXT, parts TEXT (JSON array), model TEXT, created_at INTEGER, updated_at INTEGER
            let mut msg_stmt = db
                .prepare(
                    "SELECT role, parts, model, created_at FROM messages \
                     WHERE session_id = ? ORDER BY created_at ASC",
                )
                .context("Failed to prepare messages query")?;

            let messages: Vec<ParsedMessage> = msg_stmt
                .query_map(rusqlite::params![session_id], |row| {
                    let role_str: String = row.get(0)?;
                    let parts_json: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
                    let msg_model: Option<String> = row.get(2)?;
                    let msg_ts: Option<i64> = row.get(3)?;

                    let role = match role_str.as_str() {
                        "user" | "human" => Role::User,
                        "assistant" | "model" => Role::Assistant,
                        "system" => Role::System,
                        "tool" => Role::Tool,
                        _ => Role::Info,
                    };

                    // Parse parts JSON array: [{"type":"text","text":"..."}, {"type":"tool_call","name":"...","input":{...}}, ...]
                    let content = Self::extract_content_from_parts(&parts_json);

                    // Extract tool name from tool_call parts
                    let tool_name = if role == Role::Tool || role == Role::Assistant {
                        Self::extract_tool_name_from_parts(&parts_json)
                    } else {
                        None
                    };

                    let timestamp = msg_ts.and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0));
                    let is_assistant = role == Role::Assistant;

                    Ok(ParsedMessage {
                        role,
                        content,
                        timestamp,
                        tool_name,
                        model: if is_assistant { msg_model } else { None },
                    })
                })
                .context("Failed to query messages")?
                .filter_map(|r| r.ok())
                .filter(|m| !m.content.trim().is_empty())
                .collect();

            if messages.is_empty() {
                continue;
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

            // Workspace from database filename
            let workspace = raw_path
                .file_stem()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());

            all_conversations.push(ParsedConversation {
                id: session_id,
                source: "opencode".to_string(),
                title,
                workspace,
                created_at,
                updated_at,
                model,
                messages,
                tags: Vec::new(),
            });
        }

        // Return the most recent session, or an empty one
        // For multi-session databases, the parse_vault_source function handles iteration,
        // but since each .db file is one "raw file", we merge all sessions
        if all_conversations.len() == 1 {
            Ok(all_conversations.into_iter().next().unwrap())
        } else if all_conversations.is_empty() {
            Ok(ParsedConversation {
                id: raw_path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                source: "opencode".to_string(),
                title: None,
                workspace: None,
                created_at: None,
                updated_at: None,
                model: None,
                messages: Vec::new(),
                tags: Vec::new(),
            })
        } else {
            // Multiple sessions in one DB: merge into one ParsedConversation
            // with session separators
            let mut merged_messages = Vec::new();
            let first_created = all_conversations.first().and_then(|c| c.created_at);
            let last_updated = all_conversations.last().and_then(|c| c.updated_at);
            let model = all_conversations.first().and_then(|c| c.model.clone());
            let workspace = all_conversations.first().and_then(|c| c.workspace.clone());

            for (i, conv) in all_conversations.iter().enumerate() {
                if i > 0 {
                    let separator_title = conv.title.as_deref().unwrap_or("(untitled)");
                    merged_messages.push(ParsedMessage {
                        role: Role::Info,
                        content: format!("--- Session: {} ---", separator_title),
                        timestamp: conv.created_at,
                        tool_name: None,
                        model: None,
                    });
                }
                merged_messages.extend(conv.messages.clone());
            }

            let combined_id = raw_path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            Ok(ParsedConversation {
                id: combined_id,
                source: "opencode".to_string(),
                title: Some(format!("OpenCode ({} sessions)", all_conversations.len())),
                workspace,
                created_at: first_created,
                updated_at: last_updated,
                model,
                messages: merged_messages,
                tags: Vec::new(),
            })
        }
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path.extension().is_some_and(|ext| ext == "db")
    }
}
