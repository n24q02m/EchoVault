//! Cline / Roo Code Parser
//!
//! Parses api_conversation_history.json files from Cline and Roo Code extensions.
//!
//! Format: JSON array of messages:
//! ```json
//! [
//!   {
//!     "role": "user" | "assistant",
//!     "content": [
//!       { "type": "text", "text": "..." },
//!       { "type": "tool_use", "name": "...", "input": {...} },
//!       { "type": "tool_result", "content": "..." }
//!     ]
//!   }
//! ]
//! ```
//!
//! Cline uses Anthropic-style message format with multi-part content.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

/// Cline / Roo Code Parser
pub struct ClineParser;

impl ClineParser {
    /// Extract text content from a Cline content array.
    /// Cline uses Anthropic-style multi-part content.
    fn extract_content_parts(content: &Value) -> Vec<(String, Option<String>)> {
        let mut parts = Vec::new();

        match content {
            Value::String(s) => {
                parts.push((s.clone(), None));
            }
            Value::Array(arr) => {
                for item in arr {
                    let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("text");

                    match item_type {
                        "text" => {
                            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                parts.push((text.to_string(), None));
                            }
                        }
                        "tool_use" => {
                            let tool_name = item
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("unknown_tool");
                            let input = item
                                .get("input")
                                .map(|i| serde_json::to_string_pretty(i).unwrap_or_default())
                                .unwrap_or_default();

                            // Format as a readable tool call
                            let text = if input.is_empty() || input == "{}" {
                                format!("*Called tool: {}*", tool_name)
                            } else {
                                // Truncate long inputs
                                let truncated = if input.len() > 500 {
                                    format!("{}...", &input[..500])
                                } else {
                                    input
                                };
                                format!("*Called tool: {}*\n```json\n{}\n```", tool_name, truncated)
                            };
                            parts.push((text, Some(tool_name.to_string())));
                        }
                        "tool_result" => {
                            let result_text = item
                                .get("content")
                                .and_then(|c| {
                                    c.as_str().map(|s| s.to_string()).or_else(|| {
                                        c.as_array().and_then(|arr| {
                                            arr.iter().find_map(|part| {
                                                part.get("text")
                                                    .and_then(|t| t.as_str())
                                                    .map(String::from)
                                            })
                                        })
                                    })
                                })
                                .unwrap_or_default();

                            if !result_text.is_empty() {
                                // Truncate very long tool results
                                let truncated = if result_text.len() > 1000 {
                                    format!("{}...\n*(truncated)*", &result_text[..1000])
                                } else {
                                    result_text
                                };
                                parts.push((
                                    format!("*Tool result:*\n```\n{}\n```", truncated),
                                    None,
                                ));
                            }
                        }
                        "image" => {
                            parts.push(("*[Image content]*".to_string(), None));
                        }
                        _ => {
                            // Unknown type, try to extract text
                            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                parts.push((text.to_string(), None));
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        parts
    }
}

impl Parser for ClineParser {
    fn source_name(&self) -> &'static str {
        "cline"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let content =
            std::fs::read_to_string(raw_path).context("Cannot read Cline conversation file")?;

        let json: Value =
            serde_json::from_str(&content).context("Invalid JSON in Cline conversation")?;

        let messages_arr = json
            .as_array()
            .context("Cline conversation should be a JSON array")?;

        // Session ID from parent directory name (task ID)
        let session_id = raw_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut messages = Vec::new();

        for msg_obj in messages_arr {
            let role_str = msg_obj
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("unknown");

            let role = match role_str {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                _ => Role::Info,
            };

            if let Some(content_value) = msg_obj.get("content") {
                let parts = Self::extract_content_parts(content_value);

                if parts.is_empty() {
                    continue;
                }

                // If there's a tool_use part, split into text + tool messages
                let mut text_parts = Vec::new();
                let mut tool_parts = Vec::new();

                for (text, tool_name) in &parts {
                    if tool_name.is_some() {
                        tool_parts.push((text.clone(), tool_name.clone()));
                    } else {
                        text_parts.push(text.clone());
                    }
                }

                // Add text content as one message
                let combined_text = text_parts.join("\n\n");
                if !combined_text.trim().is_empty() {
                    messages.push(ParsedMessage {
                        role: role.clone(),
                        content: combined_text,
                        timestamp: None,
                        tool_name: None,
                        model: None,
                    });
                }

                // Add tool uses as separate messages
                for (text, tool_name) in tool_parts {
                    messages.push(ParsedMessage {
                        role: Role::Tool,
                        content: text,
                        timestamp: None,
                        tool_name,
                        model: None,
                    });
                }
            }
        }

        // Title from first user message
        let title = messages.iter().find(|m| m.role == Role::User).map(|m| {
            let first_line = m.content.lines().next().unwrap_or(&m.content);
            let truncated: String = first_line.chars().take(80).collect();
            if first_line.chars().count() > 80 {
                format!("{}...", truncated)
            } else {
                truncated
            }
        });

        // Get timestamp from file metadata
        let created_at = std::fs::metadata(raw_path)
            .ok()
            .and_then(|m| m.created().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0));

        Ok(ParsedConversation {
            id: session_id,
            source: "cline".to_string(),
            title,
            workspace: None,
            created_at,
            updated_at: None,
            model: None,
            messages,
            tags: Vec::new(),
        })
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path
            .file_name()
            .is_some_and(|n| n == "api_conversation_history.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_string_content() {
        let content = json!("Hello, world!");
        let parts = ClineParser::extract_content_parts(&content);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, "Hello, world!");
        assert_eq!(parts[0].1, None);
    }

    #[test]
    fn test_extract_simple_text_content() {
        let content = json!([
            { "type": "text", "text": "Hello from array" }
        ]);
        let parts = ClineParser::extract_content_parts(&content);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, "Hello from array");
        assert_eq!(parts[0].1, None);
    }

    #[test]
    fn test_extract_tool_use() {
        let content = json!([
            {
                "type": "tool_use",
                "name": "calculator",
                "input": { "expression": "2 + 2" }
            }
        ]);
        let parts = ClineParser::extract_content_parts(&content);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].0.contains("*Called tool: calculator*"));
        // The input formatting puts it inside a json block
        assert!(parts[0].0.contains("```json"));
        assert!(parts[0].0.contains("expression"));
        assert_eq!(parts[0].1, Some("calculator".to_string()));
    }

    #[test]
    fn test_extract_tool_use_empty_input() {
        let content = json!([
            {
                "type": "tool_use",
                "name": "list_files",
                "input": {}
            }
        ]);
        let parts = ClineParser::extract_content_parts(&content);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, "*Called tool: list_files*");
        assert_eq!(parts[0].1, Some("list_files".to_string()));
    }

    #[test]
    fn test_extract_tool_result() {
        let content = json!([
            {
                "type": "tool_result",
                "content": "File listed successfully"
            }
        ]);
        let parts = ClineParser::extract_content_parts(&content);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].0.contains("*Tool result:*"));
        assert!(parts[0].0.contains("File listed successfully"));
        assert_eq!(parts[0].1, None);
    }

    #[test]
    fn test_extract_tool_result_nested_array() {
        let content = json!([
            {
                "type": "tool_result",
                "content": [
                    { "type": "text", "text": "Part 1" }
                ]
            }
        ]);
        let parts = ClineParser::extract_content_parts(&content);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].0.contains("Part 1"));
        assert_eq!(parts[0].1, None);
    }

    #[test]
    fn test_extract_image() {
        let content = json!([
            { "type": "image" }
        ]);
        let parts = ClineParser::extract_content_parts(&content);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, "*[Image content]*");
        assert_eq!(parts[0].1, None);
    }

    #[test]
    fn test_extract_unknown_type_fallback() {
        let content = json!([
            { "type": "unknown_type", "text": "Fallback text" }
        ]);
        let parts = ClineParser::extract_content_parts(&content);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, "Fallback text");
        assert_eq!(parts[0].1, None);
    }

    #[test]
    fn test_extract_empty_or_invalid() {
        let content = json!([]);
        let parts = ClineParser::extract_content_parts(&content);
        assert!(parts.is_empty());

        let content = json!(123);
        let parts = ClineParser::extract_content_parts(&content);
        assert!(parts.is_empty());
    }
}
