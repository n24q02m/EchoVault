//! Cursor AI Parser
//!
//! Cursor uses the same JSON/JSONL format as VS Code Copilot since it's a VS Code fork.
//! This parser delegates to the VS Code Copilot parser but tags output as "cursor" source.

use super::{ParsedConversation, Parser};
use anyhow::Result;
use std::path::Path;

/// Cursor AI Parser (delegates to VS Code Copilot parser format)
pub struct CursorParser;

impl Parser for CursorParser {
    fn source_name(&self) -> &'static str {
        "cursor"
    }

    fn parse(&self, raw_path: &Path) -> Result<ParsedConversation> {
        // Reuse VS Code Copilot parser since format is identical
        let copilot_parser = super::vscode_copilot::VSCodeCopilotParser;
        let mut conv = copilot_parser.parse(raw_path)?;
        // Override source to "cursor"
        conv.source = "cursor".to_string();
        Ok(conv)
    }

    fn can_parse(&self, raw_path: &Path) -> bool {
        raw_path
            .extension()
            .is_some_and(|ext| ext == "json" || ext == "jsonl")
    }
}
