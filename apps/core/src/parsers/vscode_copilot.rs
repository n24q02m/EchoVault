//! VS Code Copilot Parser
//!
//! Parses raw JSON/JSONL chat session files from GitHub Copilot in VS Code.
//!
//! Supports two formats:
//! 1. Legacy JSON: Single object with `requests[]` array
//! 2. JSONL (current): Line-oriented with `kind` field
//!    - kind=0: Session header (v contains sessionId, customTitle, creationDate)
//!    - kind=1: User message (v is string)
//!    - kind=2: Assistant response (v is string)
//!    - kind=3: Result/metadata
//!    - kind=4: Confirmation
//!    - kind=5: Follow-up

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::io::BufRead;
use std::path::Path;

/// VS Code Copilot Parser
pub struct VSCodeCopilotParser;

impl VSCodeCopilotParser {
    /// Parse a legacy JSON format session file.
    fn parse_json(&self, content: &str, file_stem: &str) -> Result<ParsedConversation> {
        let json: Value =
            serde_json::from_str(content).context("Invalid JSON in Copilot session")?;

        let session_id = json
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or(file_stem)
            .to_string();

        let title = json
            .get("customTitle")
            .and_then(|v| v.as_str())
            .map(String::from);

        let created_at = json
            .get("creationDate")
            .and_then(|v| v.as_i64())
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        let mut messages = Vec::new();

        // Parse requests array
        if let Some(requests) = json.get("requests").and_then(|v| v.as_array()) {
            for request in requests {
                // User message
                if let Some(text) = request
                    .get("message")
                    .and_then(|m| m.get("text"))
                    .and_then(|t| t.as_str())
                {
                    messages.push(ParsedMessage {
                        role: Role::User,
                        content: text.to_string(),
                        timestamp: None,
                        tool_name: None,
                        model: None,
                    });
                }

                // Assistant response
                if let Some(response) = request.get("response") {
                    let response_text = response
                        .get("value")
                        .and_then(|v| v.as_str())
                        .or_else(|| response.get("message").and_then(|m| m.as_str()))
                        .unwrap_or_default();

                    if !response_text.is_empty() {
                        let model = response
                            .get("model")
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        messages.push(ParsedMessage {
                            role: Role::Assistant,
                            content: response_text.to_string(),
                            timestamp: None,
                            tool_name: None,
                            model,
                        });
                    }
                }
            }
        }

        let title = title.or_else(|| {
            messages
                .first()
                .filter(|m| m.role == Role::User)
                .map(|m| truncate_title(&m.content))
        });

        Ok(ParsedConversation {
            id: session_id,
            source: "vscode-copilot".to_string(),
            title,
            workspace: None,
            created_at,
            updated_at: None,
            model: None,
            messages,
            tags: Vec::new(),
        })
    }

    /// Parse a JSONL format session file.
    fn parse_jsonl(&self, path: &Path, file_stem: &str) -> Result<ParsedConversation> {
        let file = std::fs::File::open(path).context("Cannot open JSONL file")?;
        let reader = std::io::BufReader::new(file);
        self.parse_jsonl_from_reader(reader, file_stem)
    }

    fn parse_jsonl_from_reader<R: BufRead>(&self, reader: R, file_stem: &str) -> Result<ParsedConversation> {


        let mut session_id = file_stem.to_string();
        let mut title: Option<String> = None;
        let mut created_at = None;
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line.context("Error reading JSONL line")?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let obj: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let kind = obj.get("kind").and_then(|v| v.as_i64()).unwrap_or(-1);

            match kind {
                0 => {
                    // Session header
                    if let Some(v) = obj.get("v") {
                        if let Some(id) = v.get("sessionId").and_then(|s| s.as_str()) {
                            session_id = id.to_string();
                        }
                        if let Some(t) = v.get("customTitle").and_then(|s| s.as_str()) {
                            title = Some(t.to_string());
                        }
                        created_at = v
                            .get("creationDate")
                            .and_then(|v| v.as_i64())
                            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());
                    }
                }
                1 => {
                    // User message
                    if let Some(text) = obj.get("v").and_then(|v| v.as_str()) {
                        if !text.trim().is_empty() {
                            messages.push(ParsedMessage {
                                role: Role::User,
                                content: text.to_string(),
                                timestamp: None,
                                tool_name: None,
                                model: None,
                            });
                        }
                    }
                }
                2 => {
                    // Assistant response
                    if let Some(text) = obj.get("v").and_then(|v| v.as_str()) {
                        if !text.trim().is_empty() {
                            messages.push(ParsedMessage {
                                role: Role::Assistant,
                                content: text.to_string(),
                                timestamp: None,
                                tool_name: None,
                                model: None,
                            });
                        }
                    }
                }
                3 => {
                    // Result/metadata — skip for now
                }
                4 => {
                    // Confirmation (tool approval) — add as info
                    if let Some(v) = obj.get("v") {
                        if let Some(text) = v.as_str() {
                            messages.push(ParsedMessage {
                                role: Role::Info,
                                content: format!("[Confirmation] {}", text),
                                timestamp: None,
                                tool_name: None,
                                model: None,
                            });
                        }
                    }
                }
                5 => {
                    // Follow-up question — treat as assistant
                    if let Some(text) = obj.get("v").and_then(|v| v.as_str()) {
                        if !text.trim().is_empty() {
                            messages.push(ParsedMessage {
                                role: Role::Assistant,
                                content: format!("**Follow-up:** {}", text),
                                timestamp: None,
                                tool_name: None,
                                model: None,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        let title = title.or_else(|| {
            messages
                .first()
                .filter(|m| m.role == Role::User)
                .map(|m| truncate_title(&m.content))
        });

        Ok(ParsedConversation {
            id: session_id,
            source: "vscode-copilot".to_string(),
            title,
            workspace: None,
            created_at,
            updated_at: None,
            model: None,
            messages,
            tags: Vec::new(),
        })
        }

}

impl Parser for VSCodeCopilotParser {
    fn source_name(&self) -> &'static str {
        "vscode-copilot"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        let file_stem = raw_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let ext = raw_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "jsonl" => self.parse_jsonl(raw_path, file_stem),
            "json" => {
                let content = std::fs::read_to_string(raw_path).context("Cannot read JSON file")?;
                self.parse_json(&content, file_stem)
            }
            _ => anyhow::bail!("Unsupported file extension: {}", ext),
        }
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path
            .extension()
            .is_some_and(|ext| ext == "json" || ext == "jsonl")
    }
}

/// Truncate a string to use as a title (max 80 chars).
fn truncate_title(s: &str) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    let truncated: String = first_line.chars().take(80).collect();
    if first_line.chars().count() > 80 {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_json_legacy() {
        let parser = VSCodeCopilotParser;
        let content = r#"{
            "sessionId": "test-session",
            "customTitle": "Test Title",
            "creationDate": 1700000000000,
            "requests": [
                {
                    "message": { "text": "Hello" },
                    "response": { "value": "Hi there" }
                }
            ]
        }"#;

        let result = parser.parse_json(content, "default").unwrap();
        assert_eq!(result.id, "test-session");
        assert_eq!(result.title.as_deref(), Some("Test Title"));
        assert_eq!(result.messages.len(), 2);
        assert_eq!(result.messages[0].role, Role::User);
        assert_eq!(result.messages[0].content, "Hello");
        assert_eq!(result.messages[1].role, Role::Assistant);
        assert_eq!(result.messages[1].content, "Hi there");
    }

    #[test]
    fn test_parse_jsonl_valid() {
        let parser = VSCodeCopilotParser;
        let content = r#"{"kind": 0, "v": {"sessionId": "jsonl-session", "customTitle": "JSONL Title", "creationDate": 1700000000000}}
{"kind": 1, "v": "User message"}
{"kind": 2, "v": "Assistant response"}
{"kind": 4, "v": "Tool confirmation"}
{"kind": 5, "v": "Follow up"}
"#;
        let reader = Cursor::new(content);
        let result = parser.parse_jsonl_from_reader(reader, "default").unwrap();

        assert_eq!(result.id, "jsonl-session");
        assert_eq!(result.title.as_deref(), Some("JSONL Title"));
        assert_eq!(result.messages.len(), 4); // User, Assistant, Info (Confirmation), Assistant (Follow-up)

        assert_eq!(result.messages[0].role, Role::User);
        assert_eq!(result.messages[0].content, "User message");

        assert_eq!(result.messages[1].role, Role::Assistant);
        assert_eq!(result.messages[1].content, "Assistant response");

        assert_eq!(result.messages[2].role, Role::Info);
        assert!(result.messages[2].content.contains("Tool confirmation"));

        assert_eq!(result.messages[3].role, Role::Assistant);
        assert!(result.messages[3].content.contains("Follow up"));
    }

    #[test]
    fn test_parse_jsonl_invalid_line() {
        let parser = VSCodeCopilotParser;
        // Middle line is invalid JSON
        let content = r#"{"kind": 1, "v": "First"}
{invalid json}
{"kind": 2, "v": "Second"}
"#;
        let reader = Cursor::new(content);
        let result = parser.parse_jsonl_from_reader(reader, "default").unwrap();

        // Should skip invalid line and continue
        assert_eq!(result.messages.len(), 2);
        assert_eq!(result.messages[0].content, "First");
        assert_eq!(result.messages[1].content, "Second");
    }

    #[test]
    fn test_parse_jsonl_empty() {
        let parser = VSCodeCopilotParser;
        let reader = Cursor::new("");
        let result = parser.parse_jsonl_from_reader(reader, "default").unwrap();

        assert_eq!(result.messages.len(), 0);
        assert_eq!(result.id, "default");
    }
}
