//! Vault management - Metadata and operations for vault.
//!
//! This module manages vault metadata and operations.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Vault metadata stored in vault.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMetadata {
    /// Metadata format version
    pub version: u32,
    /// Vault creation timestamp
    pub created_at: String,
}

impl VaultMetadata {
    /// Create new vault metadata.
    pub fn new() -> Self {
        use chrono::Utc;

        Self {
            version: 2, // Version 2: simplified, no encryption
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Load metadata from file.
    pub fn load(vault_dir: &Path) -> Result<Self> {
        let path = vault_dir.join("vault.json");
        let content = fs::read_to_string(&path)
            .context(format!("Failed to read vault.json from {:?}", path))?;
        let metadata: Self =
            serde_json::from_str(&content).context("Failed to parse vault.json")?;
        Ok(metadata)
    }

    /// Save metadata to file.
    pub fn save(&self, vault_dir: &Path) -> Result<()> {
        let path = vault_dir.join("vault.json");
        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize vault metadata")?;
        fs::write(&path, content).context(format!("Failed to write vault.json to {:?}", path))?;
        Ok(())
    }

    /// Check if vault.json exists.
    pub fn exists(vault_dir: &Path) -> bool {
        vault_dir.join("vault.json").exists()
    }
}

impl Default for VaultMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_vault_metadata_create_and_load() {
        let temp = TempDir::new().unwrap();
        let vault_dir = temp.path();

        // Create metadata
        let metadata = VaultMetadata::new();
        assert_eq!(metadata.version, 2);

        // Save
        metadata.save(vault_dir).unwrap();
        assert!(VaultMetadata::exists(vault_dir));

        // Load
        let loaded = VaultMetadata::load(vault_dir).unwrap();
        assert_eq!(loaded.version, metadata.version);
    }
}
