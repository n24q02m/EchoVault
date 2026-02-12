import os

filepath = "apps/core/src/extractors/vscode_copilot.rs"

with open(filepath, "r") as f:
    content = f.read()

# 1. Add import
if "use serde::Deserialize;" not in content:
    content = content.replace("use serde_json::Value;", "use serde_json::Value;\nuse serde::Deserialize;")

# 2. Add structs
structs_code = r'''
#[derive(Deserialize)]
struct QuickMetadata {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "creationDate")]
    creation_date: Option<i64>,
    #[serde(rename = "customTitle")]
    custom_title: Option<String>,
    #[serde(default)]
    requests: Option<FirstRequestOnly>,
}

struct FirstRequestOnly {
    text: Option<String>,
}

impl<'de> Deserialize<'de> for FirstRequestOnly {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FirstRequestVisitor;

        impl<'de> serde::de::Visitor<'de> for FirstRequestVisitor {
            type Value = FirstRequestOnly;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a list of requests")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                #[derive(Deserialize)]
                struct RequestStub {
                    message: Option<MessageStub>,
                }
                #[derive(Deserialize)]
                struct MessageStub {
                    text: Option<String>,
                }

                // Read the first element
                let first: Option<RequestStub> = seq.next_element()?;

                let text = first.and_then(|r| r.message).and_then(|m| m.text);

                // Drain the rest of the sequence without allocating
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}

                Ok(FirstRequestOnly { text })
            }
        }

        deserializer.deserialize_seq(FirstRequestVisitor)
    }
}
'''

if "struct QuickMetadata" not in content:
    # Insert before "/// VS Code Copilot Extractor"
    content = content.replace("/// VS Code Copilot Extractor", structs_code + "\n/// VS Code Copilot Extractor")

# 3. Replace extract_quick_metadata
old_method = r'''    /// Quick metadata extraction from JSON/JSONL file (only read required fields).
    fn extract_quick_metadata(
        &self,
        path: &PathBuf,
        workspace_name: &str,
    ) -> Option<SessionMetadata> {
        let is_jsonl = path.extension().is_some_and(|ext| ext == "jsonl");

        let json = if is_jsonl {
            // JSONL format: first line is kind=0 (session header), data in "v" field
            let file = std::fs::File::open(path).ok()?;
            let reader = std::io::BufReader::new(file);
            let first_line = reader.lines().next()?.ok()?;
            let wrapper: Value = serde_json::from_str(&first_line).ok()?;
            // Extract the "v" object which contains session metadata
            wrapper.get("v")?.clone()
        } else {
            // Legacy JSON format: entire file is the session object
            let content = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()?
        };

        // Get session ID from filename or JSON
        let session_id = json
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            });

        // Get title if available
        let title = json
            .get("customTitle")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                // Fallback: get text from first request (works for legacy JSON)
                json.get("requests")
                    .and_then(|r| r.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|req| req.get("message"))
                    .and_then(|msg| msg.get("text"))
                    .and_then(|t| t.as_str())
                    .map(|s| {
                        let truncated: String = s.chars().take(60).collect();
                        if s.chars().count() > 60 {
                            format!("{}...", truncated)
                        } else {
                            truncated
                        }
                    })
            })
            .or_else(|| {
                // Fallback for JSONL: read subsequent lines to find first user message
                if !is_jsonl {
                    return None;
                }
                let file = std::fs::File::open(path).ok()?;
                let reader = std::io::BufReader::new(file);
                // Skip first line (header), look for kind=1 with string value (user message)
                for line in reader.lines().skip(1).take(20).flatten() {
                    if let Ok(obj) = serde_json::from_str::<Value>(&line) {
                        if obj.get("kind").and_then(|k| k.as_i64()) == Some(1) {
                            if let Some(text) = obj.get("v").and_then(|v| v.as_str()) {
                                if !text.is_empty() && text.len() > 5 {
                                    let truncated: String = text.chars().take(60).collect();
                                    return if text.chars().count() > 60 {
                                        Some(format!("{}...", truncated))
                                    } else {
                                        Some(truncated)
                                    };
                                }
                            }
                        }
                    }
                }
                None
            });

        // Get timestamp
        let created_at = json
            .get("creationDate")
            .and_then(|v| v.as_i64())
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        // Get file size
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        Some(SessionMetadata {
            id: session_id,
            source: "vscode-copilot".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(), // Will be set after copy
            original_path: path.clone(),
            file_size,
            workspace_name: Some(workspace_name.to_string()),
            ide_origin: None,
        })
    }'''

new_method = r'''    /// Quick metadata extraction from JSON/JSONL file (only read required fields).
    fn extract_quick_metadata(
        &self,
        path: &PathBuf,
        workspace_name: &str,
    ) -> Option<SessionMetadata> {
        let is_jsonl = path.extension().is_some_and(|ext| ext == "jsonl");

        let (session_id, creation_date, custom_title, first_message_text) = if is_jsonl {
            // JSONL format: first line is kind=0 (session header), data in "v" field
            let file = std::fs::File::open(path).ok()?;
            let reader = std::io::BufReader::new(file);
            let first_line = reader.lines().next()?.ok()?;
            let wrapper: Value = serde_json::from_str(&first_line).ok()?;
            // Extract the "v" object which contains session metadata
            let v = wrapper.get("v")?;
            (
                v.get("sessionId").and_then(|s| s.as_str()).map(|s| s.to_string()),
                v.get("creationDate").and_then(|d| d.as_i64()),
                v.get("customTitle").and_then(|t| t.as_str()).map(|s| s.to_string()),
                None, // JSONL doesn't store first message in header usually, handled later
            )
        } else {
            // Legacy JSON format: use streaming parser to avoid loading full file
            let file = std::fs::File::open(path).ok()?;
            let reader = std::io::BufReader::new(file);
            let metadata: QuickMetadata = serde_json::from_reader(reader).ok()?;
            (
                metadata.session_id,
                metadata.creation_date,
                metadata.custom_title,
                metadata.requests.and_then(|r| r.text),
            )
        };

        // Get session ID from filename or JSON
        let session_id = session_id
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            });

        // Get title if available
        let title = custom_title
            .or_else(|| {
                // Fallback: get text from first request (legacy JSON)
                first_message_text.as_ref().map(|s| {
                        let truncated: String = s.chars().take(60).collect();
                        if s.chars().count() > 60 {
                            format!("{}...", truncated)
                        } else {
                            truncated
                        }
                })
            })
            .or_else(|| {
                // Fallback for JSONL: read subsequent lines to find first user message
                if !is_jsonl {
                    return None;
                }
                let file = std::fs::File::open(path).ok()?;
                let reader = std::io::BufReader::new(file);
                // Skip first line (header), look for kind=1 with string value (user message)
                for line in reader.lines().skip(1).take(20).flatten() {
                    if let Ok(obj) = serde_json::from_str::<Value>(&line) {
                        if obj.get("kind").and_then(|k| k.as_i64()) == Some(1) {
                            if let Some(text) = obj.get("v").and_then(|v| v.as_str()) {
                                if !text.is_empty() && text.len() > 5 {
                                    let truncated: String = text.chars().take(60).collect();
                                    return if text.chars().count() > 60 {
                                        Some(format!("{}...", truncated))
                                    } else {
                                        Some(truncated)
                                    };
                                }
                            }
                        }
                    }
                }
                None
            });

        // Get timestamp
        let created_at = creation_date
            .and_then(|ts| Utc.timestamp_millis_opt(ts).single());

        // Get file size
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        Some(SessionMetadata {
            id: session_id,
            source: "vscode-copilot".to_string(),
            title,
            created_at,
            vault_path: PathBuf::new(), // Will be set after copy
            original_path: path.clone(),
            file_size,
            workspace_name: Some(workspace_name.to_string()),
            ide_origin: None,
        })
    }'''

content = content.replace(old_method, new_method)

with open(filepath, "w") as f:
    f.write(content)
