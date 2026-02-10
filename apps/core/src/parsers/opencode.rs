//! OpenCode Parser
//!
//! Parses 3-tier JSON files from OpenCode v1.x (github.com/opencode-ai/opencode).
//!
//! Storage structure: ~/.local/share/opencode/storage/
//!   - session/<id>.json: {id, slug, version, projectID, directory, title, time:{created,updated}, summary}
//!   - message/<id>.json: {id, sessionID, role, time:{created}, agent, model:{providerID, modelID}, parentID, cost, tokens}
//!   - part/<id>.json:    {id, sessionID, messageID, type:"text"|"tool-invocation"|..., text:"actual content"}
//!
//! The vault stores these in: opencode/<sessionId>/{session,message,part}/*.json
//! The parser reads the session directory and reconstructs conversations.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::path::Path;

/// OpenCode Parser
pub struct OpenCodeParser;

impl Parser for OpenCodeParser {
    fn source_name(&self) -> &'static str {
        "opencode"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        // raw_path is the session directory in vault: opencode/<sessionId>/
        // It contains subdirs: session/, message/, part/

        let session_dir = raw_path.join("session");
        let message_dir = raw_path.join("message");
        let part_dir = raw_path.join("part");

        // 1. Read session metadata
        let (session_id, title, created_at, updated_at, workspace, version) =
            Self::read_session_meta(&session_dir)?;

        // 2. Read all messages for this session
        let mut messages_meta = Self::read_messages(&message_dir, &session_id);

        // Sort messages by creation time
        messages_meta.sort_by(|a, b| a.created_ms.cmp(&b.created_ms));

        // 3. Read all parts and index by messageID
        let parts = Self::read_parts(&part_dir, &session_id);

        // 4. Assemble ParsedMessages
        let mut parsed_messages = Vec::new();
        let mut model: Option<String> = None;

        for msg_meta in &messages_meta {
            // Find parts belonging to this message
            let mut msg_parts: Vec<&PartMeta> = parts
                .iter()
                .filter(|p| p.message_id == msg_meta.id)
                .collect();
            msg_parts.sort_by(|a, b| a.id.cmp(&b.id));

            let role = match msg_meta.role.as_str() {
                "user" | "human" => Role::User,
                "assistant" | "model" => Role::Assistant,
                "system" => Role::System,
                "tool" => Role::Tool,
                _ => Role::Info,
            };

            // Combine text from all parts
            let mut content_parts = Vec::new();
            let mut tool_name = None;

            for part in &msg_parts {
                match part.part_type.as_str() {
                    "text" => {
                        if !part.text.trim().is_empty() {
                            content_parts.push(part.text.clone());
                        }
                    }
                    "tool-invocation" | "tool_call" => {
                        if let Some(ref name) = part.tool_call_name {
                            tool_name = Some(name.clone());
                            content_parts.push(format!("[Tool: {}]", name));
                        }
                        if !part.text.trim().is_empty() {
                            content_parts.push(part.text.clone());
                        }
                    }
                    "tool-result" | "tool_result" => {
                        let truncated: String = part.text.chars().take(500).collect();
                        if part.text.chars().count() > 500 {
                            content_parts.push(format!("[Result] {}...", truncated));
                        } else if !part.text.is_empty() {
                            content_parts.push(format!("[Result] {}", part.text));
                        }
                    }
                    "reasoning" => {
                        if !part.text.trim().is_empty() {
                            content_parts.push(format!("**[Reasoning]** {}", part.text));
                        }
                    }
                    _ => {
                        if !part.text.trim().is_empty() {
                            content_parts.push(part.text.clone());
                        }
                    }
                }
            }

            let content = content_parts.join("\n");
            if content.trim().is_empty() {
                continue;
            }

            let timestamp = DateTime::<Utc>::from_timestamp(
                msg_meta.created_ms / 1000,
                ((msg_meta.created_ms % 1000) * 1_000_000) as u32,
            );

            // Capture model info
            let msg_model = if role == Role::Assistant {
                let m = msg_meta.model.clone();
                if model.is_none() {
                    model.clone_from(&m);
                }
                m
            } else {
                None
            };

            parsed_messages.push(ParsedMessage {
                role,
                content,
                timestamp,
                tool_name,
                model: msg_model,
            });
        }

        // Build tags
        let mut tags = Vec::new();
        if let Some(ref v) = version {
            tags.push(format!("opencode:{}", v));
        }

        Ok(ParsedConversation {
            id: session_id,
            source: "opencode".to_string(),
            title: Some(title),
            workspace,
            created_at,
            updated_at,
            model,
            messages: parsed_messages,
            tags,
        })
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        // The raw_path is a directory containing session/, message/, part/ subdirs
        raw_path.is_dir() && raw_path.join("session").is_dir()
    }
}

/// Message metadata from message/<id>.json
struct MessageMeta {
    id: String,
    role: String,
    created_ms: i64,
    model: Option<String>,
}

/// Part metadata from part/<id>.json
struct PartMeta {
    id: String,
    message_id: String,
    part_type: String,
    text: String,
    tool_call_name: Option<String>,
}

impl OpenCodeParser {
    /// Read session metadata from the first .json file in session/ directory.
    #[allow(clippy::type_complexity)]
    fn read_session_meta(
        session_dir: &Path,
    ) -> Result<(
        String,
        String,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<String>,
        Option<String>,
    )> {
        let session_file = std::fs::read_dir(session_dir)
            .context("Cannot read session directory")?
            .flatten()
            .find(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .map(|e| e.path())
            .context("No session JSON file found")?;

        let content = std::fs::read_to_string(&session_file).context("Cannot read session JSON")?;
        let json: Value = serde_json::from_str(&content).context("Invalid session JSON")?;

        let session_id = json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let title = json
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                json.get("slug")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled")
            })
            .to_string();

        let created_at = json
            .get("time")
            .and_then(|t| t.get("created"))
            .and_then(|v| v.as_i64())
            .and_then(|ms| {
                DateTime::<Utc>::from_timestamp(ms / 1000, ((ms % 1000) * 1_000_000) as u32)
            });

        let updated_at = json
            .get("time")
            .and_then(|t| t.get("updated"))
            .and_then(|v| v.as_i64())
            .and_then(|ms| {
                DateTime::<Utc>::from_timestamp(ms / 1000, ((ms % 1000) * 1_000_000) as u32)
            });

        let workspace = json.get("directory").and_then(|v| v.as_str()).map(|dir| {
            dir.replace('\\', "/")
                .rsplit('/')
                .next()
                .unwrap_or(dir)
                .to_string()
        });

        let version = json
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok((
            session_id, title, created_at, updated_at, workspace, version,
        ))
    }

    /// Read all message files matching the given session ID.
    fn read_messages(message_dir: &Path, session_id: &str) -> Vec<MessageMeta> {
        let mut messages = Vec::new();

        if !message_dir.exists() || !message_dir.is_dir() {
            return messages;
        }

        if let Ok(entries) = std::fs::read_dir(message_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|ext| ext != "json") {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(json) = serde_json::from_str::<Value>(&content) {
                        let msg_session_id =
                            json.get("sessionID").and_then(|v| v.as_str()).unwrap_or("");
                        if msg_session_id != session_id {
                            continue;
                        }

                        let id = json
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let role = json
                            .get("role")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let created_ms = json
                            .get("time")
                            .and_then(|t| t.get("created"))
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);

                        let model = json.get("model").and_then(|m| {
                            let provider =
                                m.get("providerID").and_then(|v| v.as_str()).unwrap_or("");
                            let model_id = m.get("modelID").and_then(|v| v.as_str()).unwrap_or("");
                            if !model_id.is_empty() {
                                Some(format!(
                                    "{}{}{}",
                                    provider,
                                    if !provider.is_empty() { "/" } else { "" },
                                    model_id
                                ))
                            } else {
                                None
                            }
                        });

                        messages.push(MessageMeta {
                            id,
                            role,
                            created_ms,
                            model,
                        });
                    }
                }
            }
        }

        messages
    }

    /// Read all part files matching the given session ID.
    fn read_parts(part_dir: &Path, session_id: &str) -> Vec<PartMeta> {
        let mut parts = Vec::new();

        if !part_dir.exists() || !part_dir.is_dir() {
            return parts;
        }

        if let Ok(entries) = std::fs::read_dir(part_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|ext| ext != "json") {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(json) = serde_json::from_str::<Value>(&content) {
                        let part_session_id =
                            json.get("sessionID").and_then(|v| v.as_str()).unwrap_or("");
                        if part_session_id != session_id {
                            continue;
                        }

                        let id = json
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let message_id = json
                            .get("messageID")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let part_type = json
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("text")
                            .to_string();
                        let text = json
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let tool_call_name = json
                            .get("toolName")
                            .or_else(|| json.get("name"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        parts.push(PartMeta {
                            id,
                            message_id,
                            part_type,
                            text,
                            tool_call_name,
                        });
                    }
                }
            }
        }

        parts
    }
}
