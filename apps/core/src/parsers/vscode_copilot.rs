//! VS Code Copilot Parser
//!
//! Parses raw JSON/JSONL chat session files from GitHub Copilot in VS Code.
//!
//! Supports three formats:
//! 1. Legacy JSON: Single object with `requests[]` array
//! 2. JSONL V1/V2: Line-oriented with `kind` 0-5 (simple string values)
//! 3. JSONL V3 (VS Code Insiders 2025+): Key-path based state mutations
//!    - kind=0: Session header (v = full initial state with version, creationDate)
//!    - kind=1: State updates at key path `k` (customTitle, inputState, model, etc.)
//!    - kind=2: Request/response data at key path `k`
//!      - `k=["requests"]` → new request with `message.text` (user input)
//!      - `k=["requests",N,"response"]` → response elements array
//!        - Elements without `kind` field but with `value` → assistant text
//!        - `kind:"thinking"` → reasoning content
//!        - `kind:"toolInvocationSerialized"` → tool call info

use super::{ParsedConversation, ParsedMessage, Parser, Role};
use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

/// VS Code Copilot Parser
pub struct VSCodeCopilotParser;

/// Accumulated data for a single request (user message + assistant response).
#[derive(Default)]
struct RequestData {
    user_text: String,
    model_id: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    response_texts: Vec<String>,
    thinking_texts: Vec<String>,
    tool_calls: Vec<(String, String)>, // (tool_name, description)
}

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

    /// Detect if a JSONL file uses V3 format (key-path mutations with `k` field).
    fn is_v3_format(first_line: &str) -> bool {
        if let Ok(obj) = serde_json::from_str::<Value>(first_line) {
            // V3 kind=0 has "version" field in v, OR kind=1/2 have "k" field
            if let Some(v) = obj.get("v") {
                if v.get("version").is_some() {
                    return true;
                }
            }
            obj.get("k").is_some()
        } else {
            false
        }
    }

    /// Parse a V3 JSONL format (VS Code Insiders 2025+) with key-path state mutations.
    fn parse_jsonl_v3(&self, path: &Path, file_stem: &str) -> Result<ParsedConversation> {
        let file = std::fs::File::open(path).context("Cannot open JSONL V3 file")?;
        let reader = std::io::BufReader::new(file);

        let mut session_id = file_stem.to_string();
        let mut title: Option<String> = None;
        let mut created_at = None;
        let mut requests: HashMap<usize, RequestData> = HashMap::new();
        let mut request_order: Vec<usize> = Vec::new();

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

            let kind = obj.get("kind").and_then(|v| v.as_i64()).unwrap_or(-1);
            let k = obj.get("k").and_then(|v| v.as_array());

            match kind {
                0 => {
                    // Session header - extract metadata from initial state
                    if let Some(v) = obj.get("v") {
                        if let Some(id) = v.get("sessionId").and_then(|s| s.as_str()) {
                            session_id = id.to_string();
                        }
                        if let Some(t) = v.get("customTitle").and_then(|s| s.as_str()) {
                            if !t.is_empty() {
                                title = Some(t.to_string());
                            }
                        }
                        created_at = v
                            .get("creationDate")
                            .and_then(|v| v.as_i64())
                            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());
                    }
                }
                1 => {
                    // State mutation at key path k
                    if let Some(k_path) = k {
                        Self::handle_kind1_v3(k_path, &obj, &mut title);
                    }
                }
                2 => {
                    // Request/response data
                    if let Some(k_path) = k {
                        Self::handle_kind2_v3(k_path, &obj, &mut requests, &mut request_order);
                    }
                }
                _ => {}
            }
        }

        // Build messages from accumulated request data
        let mut messages = Vec::new();
        for &req_idx in &request_order {
            if let Some(req) = requests.get(&req_idx) {
                // User message
                if !req.user_text.trim().is_empty() {
                    messages.push(ParsedMessage {
                        role: Role::User,
                        content: req.user_text.clone(),
                        timestamp: req.timestamp,
                        tool_name: None,
                        model: None,
                    });
                }

                // Tool calls (optional, brief summaries)
                for (tool_name, desc) in &req.tool_calls {
                    messages.push(ParsedMessage {
                        role: Role::Tool,
                        content: desc.clone(),
                        timestamp: None,
                        tool_name: Some(tool_name.clone()),
                        model: None,
                    });
                }

                // Assistant response text
                if !req.response_texts.is_empty() {
                    let response_text = req.response_texts.join("");
                    if !response_text.trim().is_empty() {
                        messages.push(ParsedMessage {
                            role: Role::Assistant,
                            content: response_text,
                            timestamp: None,
                            tool_name: None,
                            model: req.model_id.clone(),
                        });
                    }
                } else if !req.thinking_texts.is_empty() {
                    // Fallback: use thinking content if no explicit response text
                    let thinking = req.thinking_texts.join("\n\n");
                    if !thinking.trim().is_empty() {
                        messages.push(ParsedMessage {
                            role: Role::Assistant,
                            content: format!("*[Thinking]*\n\n{}", thinking),
                            timestamp: None,
                            tool_name: None,
                            model: req.model_id.clone(),
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

    /// Handle kind=1 V3 lines (state mutations).
    fn handle_kind1_v3(k_path: &[Value], obj: &Value, title: &mut Option<String>) {
        // k=["customTitle"] → set session title
        if k_path.len() == 1 && k_path[0].as_str() == Some("customTitle") {
            if let Some(t) = obj.get("v").and_then(|v| v.as_str()) {
                if !t.is_empty() {
                    *title = Some(t.to_string());
                }
            }
        }
        // Other kind=1 mutations (inputState, model changes) are metadata — skip
    }

    /// Handle kind=2 V3 lines (request/response data).
    fn handle_kind2_v3(
        k_path: &[Value],
        obj: &Value,
        requests: &mut HashMap<usize, RequestData>,
        request_order: &mut Vec<usize>,
    ) {
        let v = match obj.get("v") {
            Some(v) => v,
            None => return,
        };

        // k=["requests"] → new request(s) being created
        if k_path.len() == 1 && k_path[0].as_str() == Some("requests") {
            if let Some(arr) = v.as_array() {
                for req_val in arr {
                    let req_idx = request_order.len();
                    let mut req_data = RequestData::default();

                    // Extract user message
                    if let Some(text) = req_val
                        .get("message")
                        .and_then(|m| m.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        req_data.user_text = text.to_string();
                    }

                    // Extract model
                    req_data.model_id = req_val
                        .get("modelId")
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string());

                    // Extract timestamp
                    req_data.timestamp = req_val
                        .get("timestamp")
                        .and_then(|t| t.as_i64())
                        .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

                    // Extract any inline response elements
                    if let Some(resp_arr) = req_val.get("response").and_then(|r| r.as_array()) {
                        Self::extract_response_elements(resp_arr, &mut req_data);
                    }

                    request_order.push(req_idx);
                    requests.insert(req_idx, req_data);
                }
            }
        }
        // k=["requests", N, "response"] → response elements for request N
        else if k_path.len() == 3
            && k_path[0].as_str() == Some("requests")
            && k_path[2].as_str() == Some("response")
        {
            if let Some(req_idx) = k_path[1].as_u64().map(|n| n as usize) {
                if let Some(req_data) = requests.get_mut(&req_idx) {
                    if let Some(arr) = v.as_array() {
                        Self::extract_response_elements(arr, req_data);
                    }
                }
            }
        }
        // k=["requests", N, "message"] → update user message (rare)
        else if k_path.len() == 3
            && k_path[0].as_str() == Some("requests")
            && k_path[2].as_str() == Some("message")
        {
            if let Some(req_idx) = k_path[1].as_u64().map(|n| n as usize) {
                if let Some(req_data) = requests.get_mut(&req_idx) {
                    if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                        req_data.user_text = text.to_string();
                    }
                }
            }
        }
    }

    /// Extract response elements from a V3 response array.
    /// - Elements with `value` field and NO `kind` field → assistant text
    /// - `kind:"thinking"` → reasoning content
    /// - `kind:"toolInvocationSerialized"` → tool call
    fn extract_response_elements(arr: &[Value], req_data: &mut RequestData) {
        for el in arr {
            let el_kind = el.get("kind").and_then(|k| k.as_str());

            match el_kind {
                None => {
                    // No "kind" field → assistant markdown text
                    if let Some(value) = el.get("value").and_then(|v| v.as_str()) {
                        if !value.is_empty() {
                            req_data.response_texts.push(value.to_string());
                        }
                    }
                }
                Some("thinking") => {
                    if let Some(value) = el.get("value").and_then(|v| v.as_str()) {
                        if !value.is_empty() {
                            req_data.thinking_texts.push(value.to_string());
                        }
                    }
                }
                Some("toolInvocationSerialized") => {
                    let tool_id = el.get("toolId").and_then(|t| t.as_str()).unwrap_or("tool");
                    let desc = el
                        .get("pastTenseMessage")
                        .and_then(|m| m.get("value"))
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            el.get("invocationMessage")
                                .and_then(|m| m.get("value"))
                                .and_then(|v| v.as_str())
                        })
                        .unwrap_or("")
                        .to_string();
                    if !desc.is_empty() {
                        req_data.tool_calls.push((tool_id.to_string(), desc));
                    }
                }
                _ => {
                    // mcpServersStarting, inlineReference, progressMessage, etc. — skip
                }
            }
        }
    }

    /// Parse a legacy JSONL format (V1/V2) with simple kind values.
    fn parse_jsonl_v1(&self, path: &Path, file_stem: &str) -> Result<ParsedConversation> {
        let file = std::fs::File::open(path).context("Cannot open JSONL file")?;
        let reader = std::io::BufReader::new(file);

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
                    // V1/V2: User message (v is string)
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
                    // V1/V2: Assistant response (v is string)
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

    /// Parse a JSONL format session file (auto-detects V1/V2 vs V3).
    fn parse_jsonl(&self, path: &Path, file_stem: &str) -> Result<ParsedConversation> {
        // Peek first line to detect format version
        let file = std::fs::File::open(path).context("Cannot open JSONL file")?;
        let reader = std::io::BufReader::new(file);

        if let Some(Ok(first_line)) = reader.lines().next() {
            if Self::is_v3_format(first_line.trim()) {
                return self.parse_jsonl_v3(path, file_stem);
            }
        }

        self.parse_jsonl_v1(path, file_stem)
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
