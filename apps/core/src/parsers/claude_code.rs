//! Claude Code CLI Parser
//!
//! Parses JSONL conversation files from Claude Code (Anthropic CLI agent).
//!
//! Format: Each line is a JSON object representing a message:
//! ```jsonl
//! {"role":"human","content":"Fix the bug in main.rs","timestamp":"2024-01-15T10:30:00Z"}
//! {"role":"assistant","content":[{"type":"text","text":"I'll fix that..."},{"type":"tool_use","name":"write_file","input":{...}}],"timestamp":"..."}
//! {"role":"human","content":[{"type":"tool_result","tool_use_id":"...","content":"File written"}],"timestamp":"..."}
//! ```
//!
//! Claude Code uses Anthropic API message format with multi-part content arrays.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::io::BufRead;
use std::path::Path;

/// Claude Code CLI Parser
pub struct ClaudeCodeParser;

impl ClaudeCodeParser {
    /// Extract readable text from Claude Code content field.
    /// Content can be a string or Anthropic-style multi-part array.
    fn extract_content(content: &Value) -> (String, Vec<(String, String)>) {
        let mut text_parts = Vec::new();
        let mut tool_calls: Vec<(String, String)> = Vec::new(); // (tool_name, description)

        match content {
            Value::String(s) => {
                text_parts.push(s.clone());
            }
            Value::Array(arr) => {
                for item in arr {
                    let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("text");

                    match item_type {
                        "text" => {
                            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                text_parts.push(text.to_string());
                            }
                        }
                        "tool_use" => {
                            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("tool");

                            let input_desc = item
                                .get("input")
                                .map(|input| {
                                    // For common tools, extract meaningful info
                                    match name {
                                        "write_file" | "create_file" => input
                                            .get("path")
                                            .and_then(|p| p.as_str())
                                            .map(|p| format!("Write to `{}`", p))
                                            .unwrap_or_else(|| "Write file".to_string()),
                                        "read_file" => input
                                            .get("path")
                                            .and_then(|p| p.as_str())
                                            .map(|p| format!("Read `{}`", p))
                                            .unwrap_or_else(|| "Read file".to_string()),
                                        "bash" | "execute" | "run" => input
                                            .get("command")
                                            .and_then(|c| c.as_str())
                                            .map(|c| {
                                                let truncated: String =
                                                    c.chars().take(100).collect();
                                                format!("`{}`", truncated)
                                            })
                                            .unwrap_or_else(|| "Run command".to_string()),
                                        "search" | "grep" => input
                                            .get("query")
                                            .or_else(|| input.get("pattern"))
                                            .and_then(|q| q.as_str())
                                            .map(|q| format!("Search: `{}`", q))
                                            .unwrap_or_else(|| "Search".to_string()),
                                        _ => {
                                            // Generic: just show tool name
                                            format!("Called `{}`", name)
                                        }
                                    }
                                })
                                .unwrap_or_else(|| format!("Called `{}`", name));

                            tool_calls.push((name.to_string(), input_desc));
                        }
                        "tool_result" => {
                            let result = item
                                .get("content")
                                .and_then(|c| {
                                    c.as_str().map(String::from).or_else(|| {
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

                            if !result.is_empty() {
                                // Truncate long results
                                let truncated = if result.len() > 500 {
                                    format!("{}...", &result[..500])
                                } else {
                                    result
                                };
                                text_parts.push(format!(
                                    "<details>\n<summary>Tool result</summary>\n\n```\n{}\n```\n</details>",
                                    truncated
                                ));
                            }
                        }
                        "image" => {
                            text_parts.push("*[Image content]*".to_string());
                        }
                        _ => {
                            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                text_parts.push(text.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        let combined_text = text_parts.join("\n\n");
        (combined_text, tool_calls)
    }
}

impl Parser for ClaudeCodeParser {
    fn source_name(&self) -> &'static str {
        "claude-code"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let file = std::fs::File::open(raw_path).context("Cannot open Claude Code JSONL file")?;
        let reader = std::io::BufReader::new(file);

        let session_id = raw_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Project name from parent directory
        let workspace = raw_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| {
                // Decode path-encoded name: -Users-bill-project -> project
                s.split('-')
                    .rfind(|seg| !seg.is_empty())
                    .unwrap_or(s)
                    .to_string()
            });

        let mut messages = Vec::new();
        let mut first_timestamp: Option<DateTime<Utc>> = None;
        let mut last_timestamp: Option<DateTime<Utc>> = None;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let obj: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let role_str = obj
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("unknown");

            let timestamp = obj
                .get("timestamp")
                .or_else(|| obj.get("createdAt"))
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            if let Some(ts) = timestamp {
                if first_timestamp.is_none() {
                    first_timestamp = Some(ts);
                }
                last_timestamp = Some(ts);
            }

            let content = obj.get("content");
            if content.is_none() {
                continue;
            }
            let content = content.unwrap();

            let (text, tool_calls) = Self::extract_content(content);

            let role = match role_str {
                "human" | "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                _ => Role::Info,
            };

            // Add text message if not empty
            if !text.trim().is_empty() {
                messages.push(ParsedMessage {
                    role: role.clone(),
                    content: text,
                    timestamp,
                    tool_name: None,
                    model: None,
                });
            }

            // Add tool calls as separate messages
            for (tool_name, desc) in tool_calls {
                messages.push(ParsedMessage {
                    role: Role::Tool,
                    content: desc,
                    timestamp,
                    tool_name: Some(tool_name),
                    model: None,
                });
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

        Ok(ParsedConversation {
            id: session_id,
            source: "claude-code".to_string(),
            title,
            workspace,
            created_at: first_timestamp,
            updated_at: last_timestamp,
            model: None,
            messages,
            tags: Vec::new(),
        })
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path.extension().is_some_and(|ext| ext == "jsonl")
    }
}
