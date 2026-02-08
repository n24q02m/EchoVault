//! OpenAI Codex CLI Parser
//!
//! Parses JSONL rollout session files from OpenAI Codex CLI.
//!
//! Format: Each line is a JSON event in the session:
//! ```jsonl
//! {"type":"message","role":"user","content":"Fix the tests","timestamp":"..."}
//! {"type":"message","role":"assistant","content":"I'll check the tests...","timestamp":"..."}
//! {"type":"tool_call","name":"shell","input":{"command":"cargo test"},"timestamp":"..."}
//! {"type":"tool_result","output":"test result ...","timestamp":"..."}
//! ```
//!
//! Codex uses a streaming event format with type, role, and content fields.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::io::BufRead;
use std::path::Path;

/// OpenAI Codex CLI Parser
pub struct CodexParser;

impl Parser for CodexParser {
    fn source_name(&self) -> &'static str {
        "codex"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let file = std::fs::File::open(raw_path).context("Cannot open Codex JSONL file")?;
        let reader = std::io::BufReader::new(file);

        let session_id = raw_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

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

            // Parse timestamp
            let timestamp = obj
                .get("timestamp")
                .or_else(|| obj.get("created_at"))
                .and_then(|v| {
                    v.as_str()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .or_else(|| {
                            v.as_f64()
                                .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0))
                        })
                });

            if let Some(ts) = timestamp {
                if first_timestamp.is_none() {
                    first_timestamp = Some(ts);
                }
                last_timestamp = Some(ts);
            }

            let event_type = obj
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("message");

            match event_type {
                "message" | "input" | "output" => {
                    let role_str = obj.get("role").and_then(|r| r.as_str()).unwrap_or_else(|| {
                        if event_type == "input" {
                            "user"
                        } else if event_type == "output" {
                            "assistant"
                        } else {
                            "unknown"
                        }
                    });

                    let role = match role_str {
                        "user" | "human" => Role::User,
                        "assistant" | "model" => Role::Assistant,
                        "system" => Role::System,
                        _ => Role::Info,
                    };

                    let content = obj
                        .get("content")
                        .or_else(|| obj.get("text"))
                        .or_else(|| obj.get("message"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();

                    if !content.trim().is_empty() {
                        messages.push(ParsedMessage {
                            role,
                            content,
                            timestamp,
                            tool_name: None,
                            model: None,
                        });
                    }
                }
                "tool_call" | "function_call" => {
                    let tool_name = obj
                        .get("name")
                        .or_else(|| obj.get("function"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("tool")
                        .to_string();

                    let desc = obj
                        .get("input")
                        .map(|input| match tool_name.as_str() {
                            "shell" | "bash" | "execute" => input
                                .get("command")
                                .and_then(|c| c.as_str())
                                .map(|c| format!("```bash\n{}\n```", c))
                                .unwrap_or_else(|| format!("Called `{}`", tool_name)),
                            "write" | "create" | "patch" => input
                                .get("path")
                                .and_then(|p| p.as_str())
                                .map(|p| format!("Write to `{}`", p))
                                .unwrap_or_else(|| format!("Called `{}`", tool_name)),
                            _ => format!("Called `{}`", tool_name),
                        })
                        .unwrap_or_else(|| format!("Called `{}`", tool_name));

                    messages.push(ParsedMessage {
                        role: Role::Tool,
                        content: desc,
                        timestamp,
                        tool_name: Some(tool_name),
                        model: None,
                    });
                }
                "tool_result" | "function_response" => {
                    let output = obj
                        .get("output")
                        .or_else(|| obj.get("result"))
                        .or_else(|| obj.get("content"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();

                    if !output.is_empty() {
                        let truncated = if output.len() > 500 {
                            format!("{}...", &output[..500])
                        } else {
                            output.to_string()
                        };

                        messages.push(ParsedMessage {
                            role: Role::Tool,
                            content: format!("```\n{}\n```", truncated),
                            timestamp,
                            tool_name: None,
                            model: None,
                        });
                    }
                }
                "error" => {
                    let error_msg = obj
                        .get("message")
                        .or_else(|| obj.get("content"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error");

                    messages.push(ParsedMessage {
                        role: Role::Info,
                        content: format!("**Error:** {}", error_msg),
                        timestamp,
                        tool_name: None,
                        model: None,
                    });
                }
                _ => {
                    // Unknown event type, try to extract any content
                    if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
                        if !content.trim().is_empty() {
                            messages.push(ParsedMessage {
                                role: Role::Info,
                                content: content.to_string(),
                                timestamp,
                                tool_name: None,
                                model: None,
                            });
                        }
                    }
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

        Ok(ParsedConversation {
            id: session_id,
            source: "codex".to_string(),
            title,
            workspace: Some("Codex CLI".to_string()),
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
