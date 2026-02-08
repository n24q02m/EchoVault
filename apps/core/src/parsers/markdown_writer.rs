//! Markdown Writer - Serialize ParsedConversation to clean Markdown with YAML frontmatter.
//!
//! Output format:
//! ```markdown
//! ---
//! id: session-abc123
//! source: vscode-copilot
//! title: "How to implement a REST API"
//! workspace: my-project
//! created_at: 2024-01-15T10:30:00Z
//! updated_at: 2024-01-15T11:45:00Z
//! model: gpt-4
//! tags: [rust, api]
//! message_count: 12
//! ---
//!
//! ## User
//!
//! How do I implement a REST API in Rust?
//!
//! ## Assistant
//!
//! Here's how you can implement a REST API using Actix-web...
//! ```

use super::{ParsedConversation, ParsedMessage, Role};
use anyhow::Result;
use std::fmt::Write;
use std::path::Path;

/// Write a ParsedConversation to a Markdown file with YAML frontmatter.
pub fn write_markdown(conversation: &ParsedConversation, output_path: &Path) -> Result<()> {
    let content = render_markdown(conversation)?;

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(output_path, content)?;
    Ok(())
}

/// Render a ParsedConversation to a Markdown string.
pub fn render_markdown(conversation: &ParsedConversation) -> Result<String> {
    let mut out = String::with_capacity(4096);

    // YAML frontmatter
    write_frontmatter(&mut out, conversation)?;

    // Messages
    for (i, msg) in conversation.messages.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        write_message(&mut out, msg)?;
    }

    Ok(out)
}

/// Write YAML frontmatter block.
fn write_frontmatter(out: &mut String, conv: &ParsedConversation) -> Result<()> {
    out.push_str("---\n");

    writeln!(out, "id: {}", conv.id)?;
    writeln!(out, "source: {}", conv.source)?;

    if let Some(title) = &conv.title {
        writeln!(out, "title: \"{}\"", escape_yaml_string(title))?;
    }

    if let Some(workspace) = &conv.workspace {
        writeln!(out, "workspace: \"{}\"", escape_yaml_string(workspace))?;
    }

    if let Some(created_at) = &conv.created_at {
        writeln!(out, "created_at: {}", created_at.to_rfc3339())?;
    }

    if let Some(updated_at) = &conv.updated_at {
        writeln!(out, "updated_at: {}", updated_at.to_rfc3339())?;
    }

    if let Some(model) = &conv.model {
        writeln!(out, "model: {}", model)?;
    }

    if !conv.tags.is_empty() {
        writeln!(out, "tags: [{}]", conv.tags.join(", "))?;
    }

    let user_count = conv.count_by_role(&Role::User);
    let assistant_count = conv.count_by_role(&Role::Assistant);
    writeln!(out, "message_count: {}", conv.messages.len())?;
    writeln!(out, "user_messages: {}", user_count)?;
    writeln!(out, "assistant_messages: {}", assistant_count)?;

    out.push_str("---\n\n");
    Ok(())
}

/// Write a single message as a Markdown section.
fn write_message(out: &mut String, msg: &ParsedMessage) -> Result<()> {
    // Role header
    let role_label = match msg.role {
        Role::User => "User",
        Role::Assistant => "Assistant",
        Role::System => "System",
        Role::Tool => {
            if let Some(name) = &msg.tool_name {
                // Write with tool name
                write!(out, "## Tool: {}", name)?;
                if let Some(ts) = &msg.timestamp {
                    write!(out, " <small>{}</small>", ts.format("%H:%M:%S"))?;
                }
                out.push('\n');
                out.push('\n');
                write_content(out, &msg.content);
                return Ok(());
            }
            "Tool"
        }
        Role::Info => "Info",
    };

    write!(out, "## {}", role_label)?;

    // Add model info for assistant messages
    if msg.role == Role::Assistant {
        if let Some(model) = &msg.model {
            write!(out, " ({})", model)?;
        }
    }

    // Timestamp suffix
    if let Some(ts) = &msg.timestamp {
        write!(out, " <small>{}</small>", ts.format("%H:%M:%S"))?;
    }

    out.push('\n');
    out.push('\n');

    write_content(out, &msg.content);

    Ok(())
}

/// Write message content, ensuring proper Markdown formatting.
fn write_content(out: &mut String, content: &str) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        out.push_str("*(empty)*\n");
        return;
    }

    out.push_str(trimmed);
    out.push_str("\n\n");
}

/// Escape special characters in YAML strings.
fn escape_yaml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::{ParsedConversation, ParsedMessage, Role};
    use chrono::Utc;

    #[test]
    fn test_render_basic_conversation() {
        let conv = ParsedConversation {
            id: "test-123".to_string(),
            source: "vscode-copilot".to_string(),
            title: Some("Test conversation".to_string()),
            workspace: Some("my-project".to_string()),
            created_at: Some(Utc::now()),
            updated_at: None,
            model: Some("gpt-4".to_string()),
            messages: vec![
                ParsedMessage {
                    role: Role::User,
                    content: "How do I sort a vector in Rust?".to_string(),
                    timestamp: None,
                    tool_name: None,
                    model: None,
                },
                ParsedMessage {
                    role: Role::Assistant,
                    content: "You can use `vec.sort()` for in-place sorting.".to_string(),
                    timestamp: None,
                    tool_name: None,
                    model: Some("gpt-4".to_string()),
                },
            ],
            tags: vec!["rust".to_string()],
        };

        let result = render_markdown(&conv).unwrap();
        assert!(result.contains("---"));
        assert!(result.contains("id: test-123"));
        assert!(result.contains("source: vscode-copilot"));
        assert!(result.contains("## User"));
        assert!(result.contains("## Assistant (gpt-4)"));
        assert!(result.contains("sort a vector"));
    }

    #[test]
    fn test_escape_yaml_string() {
        assert_eq!(escape_yaml_string("hello \"world\""), "hello \\\"world\\\"");
        assert_eq!(escape_yaml_string("path\\to\\file"), "path\\\\to\\\\file");
    }
}
