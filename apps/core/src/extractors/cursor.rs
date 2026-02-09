//! Cursor Extractor
//!
//! Extracts chat history from Cursor AI Editor (VS Code fork).
//!
//! # Storage Locations
//!
//! - **Linux**: `~/.config/Cursor/User/workspaceStorage/<hash>/chatSessions/*.json`
//! - **macOS**: `~/Library/Application Support/Cursor/User/workspaceStorage/<hash>/chatSessions/*.json`
//! - **Windows**: `%APPDATA%\Cursor\User\workspaceStorage\<hash>\chatSessions\*.json`
//! - **WSL**: `~/.config/Cursor/User/workspaceStorage/...` (accessed from Windows via `\\wsl.localhost\Distro\...`)
//!
//! Also supports "Cursor Insiders".

use super::vscode_common::VSCodeCommon;
use super::{Extractor, ExtractorKind, SessionFile};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Cursor workspace storage relative paths (from home dir) for WSL/Linux.
const CURSOR_WSL_SUBPATHS: &[&str] = &[
    ".config/Cursor/User/workspaceStorage",
    ".config/Cursor - Insiders/User/workspaceStorage",
];

/// Cursor workspace storage relative paths (from config dir) for standard OS.
const CURSOR_CONFIG_SUBPATHS: &[&str] = &[
    "Cursor/User/workspaceStorage",
    "Cursor - Insiders/User/workspaceStorage",
];

/// Cursor Extractor
pub struct CursorExtractor {
    common: VSCodeCommon,
}

impl CursorExtractor {
    /// Create new extractor with default paths per platform.
    pub fn new() -> Self {
        Self {
            common: VSCodeCommon::new(CURSOR_CONFIG_SUBPATHS, CURSOR_WSL_SUBPATHS),
        }
    }
}

impl Default for CursorExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl Extractor for CursorExtractor {
    fn source_name(&self) -> &'static str {
        "cursor"
    }

    fn extractor_kind(&self) -> ExtractorKind {
        ExtractorKind::Ide
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
