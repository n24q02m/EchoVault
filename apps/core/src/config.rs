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

    /// Embedding configuration
    #[serde(default)]
    pub embedding: EmbeddingConfigToml,
}

/// Embedding provider preset.
///
/// Presets auto-fill api_base and model defaults so users only
/// need to pick a provider and optionally enter an API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingPreset {
    /// Local Ollama (default, no API key needed)
    #[default]
    Ollama,
    /// OpenAI API (requires API key)
    OpenAI,
    /// Any OpenAI-compatible endpoint (LiteLLM, vLLM, TGI, etc.)
    Custom,
}

impl EmbeddingPreset {
    /// Default API base URL for this preset.
    pub fn default_api_base(&self) -> &'static str {
        match self {
            Self::Ollama => "http://localhost:11434/v1",
            Self::OpenAI => "https://api.openai.com/v1",
            Self::Custom => "http://localhost:8000/v1",
        }
    }

    /// Default model name for this preset.
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Ollama => "nomic-embed-text",
            Self::OpenAI => "text-embedding-3-small",
            Self::Custom => "nomic-embed-text",
        }
    }

    /// Whether this preset requires an API key.
    pub fn requires_api_key(&self) -> bool {
        matches!(self, Self::OpenAI)
    }
}

/// Embedding configuration in TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfigToml {
    /// Provider preset (ollama, openai, custom)
    #[serde(default)]
    pub preset: EmbeddingPreset,

    /// API base URL (e.g., "http://localhost:11434/v1" for Ollama)
    #[serde(default = "default_embedding_api_base")]
    pub api_base: String,

    /// Optional API key
    #[serde(default)]
    pub api_key: Option<String>,

    /// Model name (e.g., "nomic-embed-text")
    #[serde(default = "default_embedding_model")]
    pub model: String,

    /// Target characters per chunk
    #[serde(default = "default_embedding_chunk_size")]
    pub chunk_size: usize,

    /// Overlap characters between chunks
    #[serde(default = "default_embedding_chunk_overlap")]
    pub chunk_overlap: usize,

    /// Batch size for API calls
    #[serde(default = "default_embedding_batch_size")]
    pub batch_size: usize,
}

fn default_embedding_api_base() -> String {
    "http://localhost:11434/v1".to_string()
}

fn default_embedding_model() -> String {
    "nomic-embed-text".to_string()
}

fn default_embedding_chunk_size() -> usize {
    1000
}

fn default_embedding_chunk_overlap() -> usize {
    200
}

fn default_embedding_batch_size() -> usize {
    32
}

impl Default for EmbeddingConfigToml {
    fn default() -> Self {
        let preset = EmbeddingPreset::default();
        Self {
            preset,
            api_base: preset.default_api_base().to_string(),
            api_key: None,
            model: preset.default_model().to_string(),
            chunk_size: default_embedding_chunk_size(),
            chunk_overlap: default_embedding_chunk_overlap(),
            batch_size: default_embedding_batch_size(),
        }
    }
}

impl EmbeddingConfigToml {
    /// Create config from a preset, applying its defaults.
    pub fn from_preset(preset: EmbeddingPreset) -> Self {
        Self {
            preset,
            api_base: preset.default_api_base().to_string(),
            api_key: None,
            model: preset.default_model().to_string(),
            ..Self::default()
        }
    }
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
            embedding: EmbeddingConfigToml::default(),
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

        // Restrict file permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }

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

    #[test]
    #[cfg(unix)]
    fn test_save_permissions() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("test_perms.toml");

        let config = Config::new();
        config.save(&config_path)?;

        let metadata = std::fs::metadata(&config_path)?;
        let mode = metadata.permissions().mode();
        // Check if permissions are 0o600 (rw-------)
        // We strip the file type bits (0o100000) and stick to permission bits (0o777)
        assert_eq!(
            mode & 0o777,
            0o600,
            "Config file should have 0600 permissions"
        );

        Ok(())
    }
}
