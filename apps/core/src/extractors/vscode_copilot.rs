//! VS Code Copilot Chat Extractor
//!
//! Extracts chat history from GitHub Copilot Chat extension in VS Code.
//!
//! # Storage Locations
//!
//! - **Linux**: `~/.config/Code/User/workspaceStorage/<hash>/chatSessions/*.json`
//! - **macOS**: `~/Library/Application Support/Code/User/workspaceStorage/<hash>/chatSessions/*.json`
//! - **Windows**: `%APPDATA%\Code\User\workspaceStorage\<hash>\chatSessions\*.json`
//! - **WSL**: `~/.config/Code/User/workspaceStorage/...` (accessed from Windows via `\\wsl.localhost\Distro\...`)
//!
//! Also supports "VS Code Insiders".

use super::vscode_common::VSCodeCommon;
use super::{Extractor, ExtractorKind, SessionFile};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// VS Code workspace storage relative paths (from home dir) for WSL/Linux.
const VSCODE_WSL_SUBPATHS: &[&str] = &[
    ".config/Code/User/workspaceStorage",
    ".config/Code - Insiders/User/workspaceStorage",
];

/// VS Code workspace storage relative paths (from config dir) for standard OS.
const VSCODE_CONFIG_SUBPATHS: &[&str] = &[
    "Code/User/workspaceStorage",
    "Code - Insiders/User/workspaceStorage",
];

/// VS Code Copilot Extractor
pub struct VSCodeCopilotExtractor {
    common: VSCodeCommon,
}

impl VSCodeCopilotExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        Self {
            common: VSCodeCommon::new(VSCODE_CONFIG_SUBPATHS, VSCODE_WSL_SUBPATHS),
        }
    }
}

impl Default for VSCodeCopilotExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for VSCodeCopilotExtractor {
    fn source_name(&self) -> &'static str {
        "vscode-copilot"
    }

    fn extractor_kind(&self) -> ExtractorKind {
        ExtractorKind::Extension
    }

    fn supported_ides(&self) -> &'static [&'static str] {
        &["VS Code", "VS Code Insiders"]
    }

    fn find_storage_locations(&self) -> Result<Vec<PathBuf>> {
        self.common.find_storage_locations()
    }

    fn get_workspace_name(&self, location: &Path) -> String {
        VSCodeCommon::get_workspace_name(location)
    }

    fn list_session_files(&self, location: &Path) -> Result<Vec<SessionFile>> {
        self.common.list_session_files(
            location,
            self.source_name(),
            VSCodeCommon::extract_quick_metadata,
        )
    }

    fn count_sessions(&self, location: &Path) -> Result<usize> {
        VSCodeCommon::count_sessions(location)
    }
}
