//! OpenAI Codex CLI Parser
//!
//! Parses JSONL rollout session files from OpenAI Codex CLI.
//!
//! Format: Each line is a JSON event with {timestamp, type, payload}:
//! ```jsonl
//! {"timestamp":"...","type":"session_meta","payload":{"id":"...","cwd":"...","originator":"codex_vscode","cli_version":"0.89.0","source":"vscode","model_provider":"openai","base_instructions":{...}}}
//! {"timestamp":"...","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"..."}]}}
//! {"timestamp":"...","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"..."}]}}
//! ```
//!
//! Roles: "developer" (system/tool instructions), "user", "assistant"
//! Content is an array of {type, text} objects.

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::io::BufRead;
use std::path::Path;

/// OpenAI Codex CLI Parser
pub struct CodexParser;

impl CodexParser {
    /// Extract text from a content array: [{type:"input_text", text:"..."}, ...]
    fn extract_content_text(content: &Value) -> String {
        match content {
            Value::Array(arr) => {
                let mut parts = Vec::new();
                for item in arr {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        if !text.trim().is_empty() {
                            parts.push(text.to_string());
                        }
                    }
                }
                parts.join("\n")
            }
            Value::String(s) => s.clone(),
            _ => String::new(),
        }
    }
}

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
        let mut session_cwd: Option<String> = None;
        let mut session_originator: Option<String> = None;
        let mut model_provider: Option<String> = None;

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

            // Parse timestamp (top-level field)
            let timestamp = obj
                .get("timestamp")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            if let Some(ts) = timestamp {
                if first_timestamp.is_none() {
                    first_timestamp = Some(ts);
                }
                last_timestamp = Some(ts);
            }

            let event_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match event_type {
                "session_meta" => {
                    // Extract session metadata from payload
                    if let Some(payload) = obj.get("payload") {
                        session_cwd = payload
                            .get("cwd")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        session_originator = payload
                            .get("originator")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        model_provider = payload
                            .get("model_provider")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
                "response_item" => {
                    if let Some(payload) = obj.get("payload") {
                        let payload_type =
                            payload.get("type").and_then(|t| t.as_str()).unwrap_or("");

                        match payload_type {
                            "message" => {
                                let role_str = payload
                                    .get("role")
                                    .and_then(|r| r.as_str())
                                    .unwrap_or("unknown");

                                let role = match role_str {
                                    "user" | "human" => Role::User,
                                    "assistant" | "model" => Role::Assistant,
                                    "developer" | "system" => Role::System,
                                    _ => Role::Info,
                                };

                                let content = payload
                                    .get("content")
                                    .map(Self::extract_content_text)
                                    .unwrap_or_default();

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
                            "function_call" => {
                                let tool_name = payload
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("tool")
                                    .to_string();

                                let desc = payload
                                    .get("arguments")
                                    .and_then(|a| a.as_str())
                                    .map(|args| format!("Called `{}`: {}", tool_name, args))
                                    .unwrap_or_else(|| format!("Called `{}`", tool_name));

                                messages.push(ParsedMessage {
                                    role: Role::Tool,
                                    content: desc,
                                    timestamp,
                                    tool_name: Some(tool_name),
                                    model: None,
                                });
                            }
                            "function_call_output" => {
                                let output = payload
                                    .get("output")
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
                            _ => {
                                // Unknown payload type, try to extract content
                                if let Some(content) = payload.get("content") {
                                    let text = Self::extract_content_text(content);
                                    if !text.trim().is_empty() {
                                        messages.push(ParsedMessage {
                                            role: Role::Info,
                                            content: text,
                                            timestamp,
                                            tool_name: None,
                                            model: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Unknown event type — skip
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

        // Workspace from session CWD
        let workspace = session_cwd.as_ref().map(|cwd| {
            // Extract last path component as project name
            cwd.replace('\\', "/")
                .rsplit('/')
                .next()
                .unwrap_or(cwd)
                .to_string()
        });

        // Build tags from metadata
        let mut tags = Vec::new();
        if let Some(ref orig) = session_originator {
            tags.push(format!("originator:{}", orig));
        }
        if let Some(ref provider) = model_provider {
            tags.push(format!("provider:{}", provider));
        }

        Ok(ParsedConversation {
            id: session_id,
            source: "codex".to_string(),
            title,
            workspace,
            created_at: first_timestamp,
            updated_at: last_timestamp,
            model: model_provider,
            messages,
            tags,
        })
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path.extension().is_some_and(|ext| ext == "jsonl")
    }
}
