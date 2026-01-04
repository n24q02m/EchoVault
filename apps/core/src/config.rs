//! Config module - Manages EchoVault configuration (echovault.toml).
//!
//! Configuration file contains:
//! - Rclone sync settings
//! - Vault path
//! - Other settings

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Cloud sync configuration via Rclone.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    /// Remote name in rclone config (e.g., "echovault")
    pub remote_name: Option<String>,
    /// Folder name on cloud (default: "EchoVault")
    #[serde(default = "default_folder_name")]
    pub folder_name: String,
}

fn default_folder_name() -> String {
    "EchoVault".to_string()
}

/// Extractors configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractorsConfig {
    /// Enabled sources (default: all)
    #[serde(default)]
    pub enabled_sources: Vec<String>,
}

/// Main EchoVault configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Config version (for future migrations)
    #[serde(default = "default_version")]
    pub version: u32,

    /// Whether setup is complete
    #[serde(default)]
    pub setup_complete: bool,

    /// Path to vault directory
    pub vault_path: PathBuf,

    /// Sync configuration
    #[serde(default)]
    pub sync: SyncConfig,

    /// Extractors configuration
    #[serde(default)]
    pub extractors: ExtractorsConfig,

    /// Export path for session exports
    #[serde(default)]
    pub export_path: Option<PathBuf>,
}

fn default_version() -> u32 {
    2 // Version 2: simplified, Rclone-only
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            setup_complete: false,
            vault_path: default_vault_path(),
            sync: SyncConfig::default(),
            extractors: ExtractorsConfig::default(),
            export_path: None,
        }
    }
}

/// Get default vault path.
pub fn default_vault_path() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join("echovault").join("vault"))
        .unwrap_or_else(|| PathBuf::from("./vault"))
}

/// Get default config directory (~/.config/echovault/).
pub fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("echovault"))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Get default config file path.
pub fn default_config_path() -> PathBuf {
    default_config_dir().join("echovault.toml")
}

#[allow(dead_code)]
impl Config {
    /// Create new config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create config with specific vault path.
    pub fn with_vault_path(vault_path: PathBuf) -> Self {
        Self {
            vault_path,
            ..Self::default()
        }
    }

    /// Load config from file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Cannot parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Load config from default path.
    pub fn load_default() -> Result<Self> {
        let path = default_config_path();
        if path.exists() {
            Self::load(&path)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to file.
    pub fn save(&self, path: &Path) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content =
            toml::to_string_pretty(self).with_context(|| "Cannot serialize config to TOML")?;

        std::fs::write(path, content)
            .with_context(|| format!("Cannot write config file: {}", path.display()))?;

        Ok(())
    }

    /// Save config to default path.
    pub fn save_default(&self) -> Result<PathBuf> {
        let path = default_config_path();
        self.save(&path)?;
        Ok(path)
    }

    /// Check if config is initialized (has a remote).
    pub fn is_initialized(&self) -> bool {
        self.sync.remote_name.is_some()
    }

    /// Get vault directory path.
    pub fn vault_dir(&self) -> &Path {
        &self.vault_path
    }

    /// Get index database path.
    pub fn index_db_path(&self) -> PathBuf {
        self.vault_path.join("index.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, 2);
        assert!(!config.is_initialized());
    }

    #[test]
    fn test_save_and_load() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("test.toml");

        let mut config = Config::new();
        config.sync.remote_name = Some("echovault".to_string());
        config.save(&config_path)?;

        let loaded = Config::load(&config_path)?;
        assert!(loaded.is_initialized());
        assert_eq!(loaded.sync.remote_name, Some("echovault".to_string()));

        Ok(())
    }
}
