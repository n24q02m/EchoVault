//! Cursor AI Parser
//!
//! Parses chat conversations from Cursor's state.vscdb SQLite database.
//!
//! The database (`cursorDiskKV` table) stores:
//! - `composerData:<uuid>` — JSON with conversation metadata (name, createdAt, status,
//!   fullConversationHeadersOnly, conversationMap)
//! - `agentKv:blob:<hash>` — JSON with individual messages (OpenAI-compatible:
//!   `{role:"system"|"user"|"assistant", content:string|[{type,text}]}`)
//!
//! The parser receives a .vscdb file path. It reads the specific composer by ID
//! (the ID is stored in the ParsedConversation.id from the extractor) and reconstructs
//! the conversation from conversationMap or fullConversationHeadersOnly references.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Cursor AI Parser
pub struct CursorParser;

impl Parser for CursorParser {
    fn source_name(&self) -> &'static str {
        "cursor"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        // raw_path is the .vscdb file in vault: cursor/<variant>.vscdb
        // We parse ALL composers in this db and merge them.
        // For single-composer mode, the orchestrator should filter by ID.

        let db = rusqlite::Connection::open_with_flags(
            raw_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Cannot open Cursor state.vscdb")?;

        // Load all agentKv:blob entries into a lookup map
        let blob_map = Self::load_agent_blobs(&db)?;

        // Load all composerData entries
        let mut stmt = db
            .prepare("SELECT key, value FROM cursorDiskKV WHERE key LIKE 'composerData:%'")
            .context("Failed to prepare composerData query")?;

        let composers: Vec<(String, String)> = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                Ok((key, value))
            })
            .context("Failed to query composerData")?
            .filter_map(|r| r.ok())
            .collect();

        if composers.is_empty() {
            return Ok(ParsedConversation {
                id: raw_path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                source: "cursor".to_string(),
                title: None,
                workspace: None,
                created_at: None,
                updated_at: None,
                model: None,
                messages: Vec::new(),
                tags: Vec::new(),
            });
        }

        // Parse each composer and merge
        let mut all_conversations: Vec<ParsedConversation> = Vec::new();

        for (_key, value) in &composers {
            if let Ok(conv) = Self::parse_composer(value, &blob_map) {
                if !conv.messages.is_empty() {
                    all_conversations.push(conv);
                }
            }
        }

        // Sort by creation time
        all_conversations.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        if all_conversations.len() == 1 {
            Ok(all_conversations.into_iter().next().unwrap())
        } else if all_conversations.is_empty() {
            Ok(ParsedConversation {
                id: raw_path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                source: "cursor".to_string(),
                title: None,
                workspace: None,
                created_at: None,
                updated_at: None,
                model: None,
                messages: Vec::new(),
                tags: Vec::new(),
            })
        } else {
            // Multiple composers: merge with separators
            let mut merged_messages = Vec::new();
            let first_created = all_conversations.first().and_then(|c| c.created_at);
            let last_updated = all_conversations.last().and_then(|c| c.updated_at);
            let model = all_conversations.iter().find_map(|c| c.model.clone());

            for (i, conv) in all_conversations.iter().enumerate() {
                if i > 0 {
                    let separator_title = conv.title.as_deref().unwrap_or("(untitled)");
                    merged_messages.push(ParsedMessage {
                        role: Role::Info,
                        content: format!("--- Composer: {} ---", separator_title),
                        timestamp: conv.created_at,
                        tool_name: None,
                        model: None,
                    });
                }
                merged_messages.extend(conv.messages.clone());
            }

            Ok(ParsedConversation {
                id: format!(
                    "cursor-{}",
                    raw_path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("db")
                ),
                source: "cursor".to_string(),
                title: Some(format!("Cursor ({} composers)", all_conversations.len())),
                workspace: None,
                created_at: first_created,
                updated_at: last_updated,
                model,
                messages: merged_messages,
                tags: Vec::new(),
            })
        }
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path.extension().is_some_and(|ext| ext == "vscdb")
    }
}

impl CursorParser {
    /// Load all agentKv:blob entries from the database into a HashMap.
    fn load_agent_blobs(db: &rusqlite::Connection) -> Result<HashMap<String, String>> {
        let mut stmt = db
            .prepare("SELECT key, value FROM cursorDiskKV WHERE key LIKE 'agentKv:blob:%'")
            .context("Failed to prepare agentKv query")?;

        let blobs: HashMap<String, String> = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                Ok((key, value))
            })
            .context("Failed to query agentKv blobs")?
            .filter_map(|r| r.ok())
            .collect();

        Ok(blobs)
    }

    /// Parse a single composer from its JSON blob.
    fn parse_composer(
        json_str: &str,
        blob_map: &HashMap<String, String>,
    ) -> Result<ParsedConversation> {
        let json: Value = serde_json::from_str(json_str).context("Invalid composerData JSON")?;

        let composer_id = json
            .get("composerId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let title = json
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let created_at = json
            .get("createdAt")
            .and_then(|v| v.as_i64())
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        let is_agentic = json
            .get("isAgentic")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let status = json
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Try to extract model from modelConfig
        let model = json
            .get("modelConfig")
            .and_then(|mc| mc.get("model"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Extract messages from conversationMap or fullConversationHeadersOnly
        let mut messages = Vec::new();

        // Strategy 1: Try conversationMap (has actual message content inline)
        if let Some(conv_map) = json.get("conversationMap").and_then(|v| v.as_object()) {
            let mut map_messages: Vec<(u64, ParsedMessage)> = Vec::new();

            for (_bubble_id, bubble) in conv_map {
                if let Some(msg) = Self::parse_bubble_message(bubble) {
                    let order = bubble
                        .get("timingInfo")
                        .and_then(|t| t.get("clientStartTime"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    map_messages.push((order, msg));
                }
            }

            // Sort by timing
            map_messages.sort_by_key(|(order, _)| *order);
            messages = map_messages.into_iter().map(|(_, msg)| msg).collect();
        }

        // Strategy 2: If conversationMap is empty, try fullConversationHeadersOnly
        // These headers reference agentKv:blob entries
        if messages.is_empty() {
            if let Some(headers) = json
                .get("fullConversationHeadersOnly")
                .and_then(|v| v.as_array())
            {
                for header in headers {
                    let bubble_id = header.get("bubbleId").and_then(|v| v.as_str());
                    let msg_type = header.get("type").and_then(|v| v.as_str());

                    // Try to find corresponding agentKv blob
                    if let Some(bid) = bubble_id {
                        // Try various key patterns
                        for prefix in &["agentKv:blob:", "agentKv:blob:"] {
                            let key = format!("{}{}", prefix, bid);
                            if let Some(blob_value) = blob_map.get(&key) {
                                if let Some(msg) = Self::parse_agent_blob(blob_value) {
                                    messages.push(msg);
                                    break;
                                }
                            }
                        }
                    }

                    // If no blob found, create a placeholder from header type
                    if messages.len()
                        < headers
                            .iter()
                            .position(|h| std::ptr::eq(h, header))
                            .unwrap_or(0)
                            + 1
                    {
                        if let Some(t) = msg_type {
                            let role = match t {
                                "user" | "human" => Role::User,
                                "ai" | "assistant" => Role::Assistant,
                                _ => Role::Info,
                            };
                            messages.push(ParsedMessage {
                                role,
                                content: format!("[{} message — content in blob storage]", t),
                                timestamp: None,
                                tool_name: None,
                                model: None,
                            });
                        }
                    }
                }
            }
        }

        // Strategy 3: If still empty, scan all blobs for any that might relate
        // (less precise, but better than nothing)
        if messages.is_empty() && !blob_map.is_empty() {
            // Try scanning blobs that look like they have conversation content
            let mut blob_messages: Vec<ParsedMessage> = blob_map
                .values()
                .filter_map(|v| Self::parse_agent_blob(v))
                .collect();

            // Only use if we found a reasonable number
            if blob_messages.len() <= 100 {
                messages.append(&mut blob_messages);
            }
        }

        // Filter empty messages
        messages.retain(|m| !m.content.trim().is_empty());

        // Build tags
        let mut tags = Vec::new();
        if is_agentic {
            tags.push("agentic".to_string());
        }
        tags.push(format!("status:{}", status));

        Ok(ParsedConversation {
            id: composer_id,
            source: "cursor".to_string(),
            title,
            workspace: None,
            created_at,
            updated_at: None,
            model,
            messages,
            tags,
        })
    }

    /// Parse a message from a conversationMap bubble entry.
    fn parse_bubble_message(bubble: &Value) -> Option<ParsedMessage> {
        let role_str = bubble.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let role = match role_str {
            "user" | "human" => Role::User,
            "ai" | "assistant" => Role::Assistant,
            "system" => Role::System,
            "tool" => Role::Tool,
            _ => return None,
        };

        let content = Self::extract_content_from_value(bubble.get("text"))
            .or_else(|| Self::extract_content_from_value(bubble.get("content")))?;

        if content.trim().is_empty() {
            return None;
        }

        let timestamp = bubble
            .get("timingInfo")
            .and_then(|t| t.get("clientStartTime"))
            .and_then(|v| v.as_i64())
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        Some(ParsedMessage {
            role: role.clone(),
            content,
            timestamp,
            tool_name: None,
            model: if role == Role::Assistant {
                bubble
                    .get("modelType")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            },
        })
    }

    /// Parse a message from an agentKv:blob value (OpenAI-compatible format).
    fn parse_agent_blob(blob_str: &str) -> Option<ParsedMessage> {
        let json: Value = serde_json::from_str(blob_str).ok()?;

        let role_str = json.get("role").and_then(|v| v.as_str())?;
        let role = match role_str {
            "user" | "human" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            "tool" | "function" => Role::Tool,
            _ => Role::Info,
        };

        let content = Self::extract_content_from_value(json.get("content"))?;
        if content.trim().is_empty() {
            return None;
        }

        Some(ParsedMessage {
            role,
            content,
            timestamp: None,
            tool_name: json
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            model: None,
        })
    }

    /// Extract text content from a JSON value that may be a string or an array of content parts.
    fn extract_content_from_value(value: Option<&Value>) -> Option<String> {
        let v = value?;

        match v {
            Value::String(s) => {
                if s.is_empty() {
                    None
                } else {
                    Some(s.clone())
                }
            }
            Value::Array(arr) => {
                let parts: Vec<String> = arr
                    .iter()
                    .filter_map(|item| {
                        let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                        match item_type {
                            "text" => item
                                .get("text")
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string()),
                            "image_url" | "image" => Some("[Image]".to_string()),
                            _ => item
                                .get("text")
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string()),
                        }
                    })
                    .collect();
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join("\n"))
                }
            }
            _ => None,
        }
    }
}
