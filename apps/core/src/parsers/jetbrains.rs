//! JetBrains AI Assistant Parser
//!
//! Parses XML workspace files from JetBrains IDEs.
//!
//! Known components containing AI chat history:
//! - `AiAssistantConversation` (newer format)
//! - `ChatSessionStateTemp` (older AI Assistant plugin)
//!
//! ## Format: AiAssistantConversation (primary)
//! ```xml
//! <component name="AiAssistantConversation">
//!   <conversations>
//!     <conversation id="..." timestamp="...">
//!       <messages>
//!         <message role="user" content="..." timestamp="..." />
//!         <message role="assistant" content="..." timestamp="..." model="..." />
//!       </messages>
//!     </conversation>
//!   </conversations>
//! </component>
//! ```
//!
//! ## Format: ChatSessionStateTemp (legacy fallback)
//! Uses `<option name="..." value="..."/>` style attributes.
//!
//! Parsing is defensive to handle both formats and version variations.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::Path;

/// JetBrains AI Assistant Parser
pub struct JetBrainsParser;

impl JetBrainsParser {
    /// Parse all AI conversations from an XML workspace file.
    fn parse_workspace_xml(raw_path: &Path) -> Result<Vec<ParsedConversation>> {
        let content =
            std::fs::read_to_string(raw_path).context("Cannot read JetBrains workspace XML")?;

        let mut conversations = Vec::new();

        // Try primary format: AiAssistantConversation
        conversations.extend(Self::parse_ai_assistant_conversations(&content, raw_path));

        // Try legacy format: ChatSessionStateTemp
        if conversations.is_empty() {
            conversations.extend(Self::parse_chat_session_state(&content, raw_path));
        }

        Ok(conversations)
    }

    /// Parse the AiAssistantConversation component format.
    ///
    /// Structure:
    /// ```xml
    /// <component name="AiAssistantConversation">
    ///   <conversations>
    ///     <conversation id="..." timestamp="...">
    ///       <messages>
    ///         <message role="user" content="..." timestamp="..." />
    ///       </messages>
    ///     </conversation>
    ///   </conversations>
    /// </component>
    /// ```
    fn parse_ai_assistant_conversations(content: &str, raw_path: &Path) -> Vec<ParsedConversation> {
        let component_start = match content.find("\"AiAssistantConversation\"") {
            Some(pos) => pos,
            None => return Vec::new(),
        };

        // Find the component boundaries
        let search_start = content[..component_start]
            .rfind("<component")
            .unwrap_or(component_start);
        let component_end = content[search_start..]
            .find("</component>")
            .map(|p| search_start + p + "</component>".len())
            .unwrap_or(content.len());
        let component_xml = &content[search_start..component_end];

        let mut conversations = Vec::new();

        // Split on <conversation markers
        let conv_chunks: Vec<&str> = component_xml.split("<conversation ").skip(1).collect();

        for chunk in conv_chunks {
            let conv_end = chunk.find("</conversation>").unwrap_or(chunk.len());
            let conv_xml = &chunk[..conv_end];

            // Extract conversation attributes
            let conv_id = Self::extract_attr(conv_xml, "id")
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let conv_timestamp =
                Self::extract_attr(conv_xml, "timestamp").and_then(|ts| Self::parse_timestamp(&ts));
            let title = Self::extract_attr(conv_xml, "title");

            // Parse messages
            let messages = Self::parse_messages_from_chunk(conv_xml);

            if messages.is_empty() {
                continue;
            }

            let title = title.or_else(|| Self::title_from_messages(&messages));

            let workspace = raw_path
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());

            conversations.push(ParsedConversation {
                id: format!("jetbrains-{}", conv_id),
                source: "jetbrains".to_string(),
                title,
                workspace,
                created_at: conv_timestamp,
                updated_at: conv_timestamp,
                model: messages.iter().rev().find_map(|m| m.model.clone()),
                messages,
                tags: Vec::new(),
            });
        }

        conversations
    }

    /// Parse <message> elements from a conversation chunk.
    fn parse_messages_from_chunk(conv_xml: &str) -> Vec<ParsedMessage> {
        let mut messages = Vec::new();

        // Split on <message markers
        let msg_chunks: Vec<&str> = conv_xml.split("<message ").skip(1).collect();

        for chunk in msg_chunks {
            let msg_end = chunk
                .find("/>")
                .or_else(|| chunk.find("</message>"))
                .unwrap_or(chunk.len());
            let msg_xml = &chunk[..msg_end];

            let role_str = Self::extract_attr(msg_xml, "role").unwrap_or_default();
            let content = Self::extract_attr(msg_xml, "content")
                .map(|c| Self::decode_xml_entities(&c))
                .unwrap_or_default();
            let model = Self::extract_attr(msg_xml, "model");
            let timestamp =
                Self::extract_attr(msg_xml, "timestamp").and_then(|ts| Self::parse_timestamp(&ts));

            if content.trim().is_empty() {
                continue;
            }

            let role = match role_str.to_lowercase().as_str() {
                "user" | "human" => Role::User,
                "assistant" | "model" | "ai" => Role::Assistant,
                "system" => Role::System,
                "tool" => Role::Tool,
                _ => Role::Info,
            };

            messages.push(ParsedMessage {
                role,
                content,
                timestamp,
                tool_name: None,
                model,
            });
        }

        messages
    }

    /// Parse the legacy ChatSessionStateTemp component format.
    ///
    /// Uses `<option name="..." value="..."/>` pattern for fields.
    fn parse_chat_session_state(content: &str, raw_path: &Path) -> Vec<ParsedConversation> {
        let component_start = match content.find("\"ChatSessionStateTemp\"") {
            Some(pos) => pos,
            None => return Vec::new(),
        };

        let search_start = content[..component_start]
            .rfind("<component")
            .unwrap_or(component_start);
        let component_end = content[search_start..]
            .find("</component>")
            .map(|p| search_start + p + "</component>".len())
            .unwrap_or(content.len());
        let component_xml = &content[search_start..component_end];

        let mut conversations = Vec::new();

        // Look for session/chat markers - various element names used
        for session_marker in &["<ChatSession", "<session ", "<conversation ", "<chat "] {
            let session_chunks: Vec<&str> = component_xml.split(session_marker).skip(1).collect();

            for chunk in &session_chunks {
                let session_end = chunk
                    .find("</ChatSession>")
                    .or_else(|| chunk.find("</session>"))
                    .or_else(|| chunk.find("</conversation>"))
                    .or_else(|| chunk.find("</chat>"))
                    .unwrap_or(chunk.len());
                let session_xml = &chunk[..session_end];

                let session_id = Self::extract_attr(session_xml, "id")
                    .or_else(|| Self::extract_option_value(session_xml, "id"))
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                let title = Self::extract_option_value(session_xml, "title")
                    .or_else(|| Self::extract_attr(session_xml, "title"));

                let timestamp = Self::extract_option_value(session_xml, "timestamp")
                    .or_else(|| Self::extract_attr(session_xml, "timestamp"))
                    .and_then(|ts| Self::parse_timestamp(&ts));

                // Parse messages using <ChatMessage>, <message>, etc.
                let mut messages = Vec::new();

                for msg_marker in &["<ChatMessage", "<message "] {
                    let msg_chunks: Vec<&str> = session_xml.split(msg_marker).skip(1).collect();

                    for msg_chunk in &msg_chunks {
                        let msg_end = msg_chunk
                            .find("</ChatMessage>")
                            .or_else(|| msg_chunk.find("</message>"))
                            .or_else(|| msg_chunk.find("/>"))
                            .unwrap_or(msg_chunk.len());
                        let msg_xml = &msg_chunk[..msg_end];

                        let role_str = Self::extract_attr(msg_xml, "role")
                            .or_else(|| Self::extract_option_value(msg_xml, "role"))
                            .unwrap_or_default();

                        let content = Self::extract_attr(msg_xml, "content")
                            .or_else(|| Self::extract_option_value(msg_xml, "content"))
                            .or_else(|| Self::extract_option_value(msg_xml, "text"))
                            .map(|c| Self::decode_xml_entities(&c))
                            .unwrap_or_default();

                        let model = Self::extract_attr(msg_xml, "model")
                            .or_else(|| Self::extract_option_value(msg_xml, "model"));

                        if content.trim().is_empty() {
                            continue;
                        }

                        let role = match role_str.to_lowercase().as_str() {
                            "user" | "human" => Role::User,
                            "assistant" | "model" | "ai" => Role::Assistant,
                            "system" => Role::System,
                            "tool" => Role::Tool,
                            _ => Role::Info,
                        };

                        messages.push(ParsedMessage {
                            role,
                            content,
                            timestamp: Self::extract_attr(msg_xml, "timestamp")
                                .or_else(|| Self::extract_option_value(msg_xml, "timestamp"))
                                .and_then(|ts| Self::parse_timestamp(&ts)),
                            tool_name: None,
                            model,
                        });
                    }

                    if !messages.is_empty() {
                        break;
                    }
                }

                if messages.is_empty() {
                    continue;
                }

                let title = title.or_else(|| Self::title_from_messages(&messages));

                let workspace = raw_path
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string());

                conversations.push(ParsedConversation {
                    id: format!("jetbrains-{}", session_id),
                    source: "jetbrains".to_string(),
                    title,
                    workspace,
                    created_at: timestamp,
                    updated_at: timestamp,
                    model: messages.iter().rev().find_map(|m| m.model.clone()),
                    messages,
                    tags: Vec::new(),
                });
            }

            if !conversations.is_empty() {
                break;
            }
        }

        conversations
    }

    /// Extract XML attribute value: `attr="value"` -> `value`
    fn extract_attr(xml: &str, attr_name: &str) -> Option<String> {
        let pattern = format!("{}=\"", attr_name);
        if let Some(pos) = xml.find(&pattern) {
            let rest = &xml[pos + pattern.len()..];
            if let Some(end) = rest.find('"') {
                let value = &rest[..end];
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    /// Extract option element value: `<option name="key" value="val"/>` -> `val`
    fn extract_option_value(xml: &str, name: &str) -> Option<String> {
        let pattern = format!("name=\"{}\"", name);
        if let Some(pos) = xml.find(&pattern) {
            // Look for value="..." near this position
            let rest = &xml[pos..];
            let nearby = &rest[..rest.len().min(200)];
            if let Some(val_pos) = nearby.find("value=\"") {
                let val_rest = &nearby[val_pos + "value=\"".len()..];
                if let Some(end) = val_rest.find('"') {
                    let value = &val_rest[..end];
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
        None
    }

    /// Parse a timestamp string - handles Unix millis, Unix seconds, and ISO 8601.
    fn parse_timestamp(ts: &str) -> Option<DateTime<Utc>> {
        // Try as Unix milliseconds (common in JetBrains)
        if let Ok(millis) = ts.parse::<i64>() {
            if millis > 1_000_000_000_000 {
                // Milliseconds
                return DateTime::<Utc>::from_timestamp(
                    millis / 1000,
                    (millis % 1000) as u32 * 1_000_000,
                );
            } else if millis > 0 {
                // Seconds
                return DateTime::<Utc>::from_timestamp(millis, 0);
            }
        }

        // Try ISO 8601 / RFC 3339
        if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
            return Some(dt.with_timezone(&Utc));
        }

        None
    }

    /// Decode common XML entities in content.
    fn decode_xml_entities(s: &str) -> String {
        s.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&#10;", "\n")
            .replace("&#13;", "\r")
            .replace("&#9;", "\t")
    }

    /// Generate title from first user message.
    fn title_from_messages(messages: &[ParsedMessage]) -> Option<String> {
        messages.iter().find(|m| m.role == Role::User).map(|m| {
            let first_line = m.content.lines().next().unwrap_or(&m.content);
            let truncated: String = first_line.chars().take(80).collect();
            if first_line.chars().count() > 80 {
                format!("{}...", truncated)
            } else {
                truncated
            }
        })
    }
}

impl Parser for JetBrainsParser {
    fn source_name(&self) -> &'static str {
        "jetbrains"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let conversations = Self::parse_workspace_xml(raw_path)?;

        if conversations.is_empty() {
            return Ok(ParsedConversation {
                id: raw_path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                source: "jetbrains".to_string(),
                title: None,
                workspace: None,
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

        // Multiple conversations: merge with separators
        let mut merged_messages = Vec::new();
        let first_created = conversations.first().and_then(|c| c.created_at);
        let last_updated = conversations.last().and_then(|c| c.updated_at);
        let model = conversations.first().and_then(|c| c.model.clone());
        let workspace = conversations.first().and_then(|c| c.workspace.clone());

        for (i, conv) in conversations.iter().enumerate() {
            if i > 0 {
                let sep_title = conv.title.as_deref().unwrap_or("(untitled)");
                merged_messages.push(ParsedMessage {
                    role: Role::Info,
                    content: format!("--- Conversation: {} ---", sep_title),
                    timestamp: conv.created_at,
                    tool_name: None,
                    model: None,
                });
            }
            merged_messages.extend(conv.messages.clone());
        }

        Ok(ParsedConversation {
            id: format!(
                "jetbrains-{}",
                raw_path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
            ),
            source: "jetbrains".to_string(),
            title: Some(format!(
                "JetBrains AI ({} conversations)",
                conversations.len()
            )),
            workspace,
            created_at: first_created,
            updated_at: last_updated,
            model,
            messages: merged_messages,
            tags: Vec::new(),
        })
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path.extension().is_some_and(|ext| ext == "xml")
    }
}
