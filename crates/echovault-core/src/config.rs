//! Config module - Quản lý cấu hình EchoVault (echovault.toml).
//!
//! File cấu hình chứa:
//! - Thông tin sync (remote repo, auth method)
//! - Đường dẫn vault
//! - Các settings khác

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Cấu hình sync với cloud
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    /// URL của remote repository (GitHub)
    pub remote: Option<String>,
    /// Tên repo (không có URL đầy đủ)
    pub repo_name: Option<String>,
    /// Provider type: github, google_drive, s3
    #[serde(default = "default_provider")]
    pub provider: String,
}

fn default_provider() -> String {
    "github".to_string()
}

/// Cấu hình encryption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Có bật mã hóa không
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// Cấu hình compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Có bật nén không
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
        }
    }
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
        }
    }
}

/// Cấu hình extractors
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractorsConfig {
    /// Các sources được bật (mặc định: tất cả)
    #[serde(default)]
    pub enabled_sources: Vec<String>,
}

/// Cấu hình chính của EchoVault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Phiên bản config (để migrate trong tương lai)
    #[serde(default = "default_version")]
    pub version: u32,

    /// Đã hoàn thành setup chưa
    #[serde(default)]
    pub setup_complete: bool,

    /// Đường dẫn đến vault directory
    pub vault_path: PathBuf,

    /// Cấu hình sync
    #[serde(default)]
    pub sync: SyncConfig,

    /// Cấu hình encryption
    #[serde(default)]
    pub encryption: EncryptionConfig,

    /// Cấu hình compression
    #[serde(default)]
    pub compression: CompressionConfig,

    /// Cấu hình extractors
    #[serde(default)]
    pub extractors: ExtractorsConfig,
}

fn default_version() -> u32 {
    1
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            setup_complete: false,
            vault_path: default_vault_path(),
            sync: SyncConfig::default(),
            encryption: EncryptionConfig::default(),
            compression: CompressionConfig::default(),
            extractors: ExtractorsConfig::default(),
        }
    }
}

/// Lấy đường dẫn vault mặc định
pub fn default_vault_path() -> PathBuf {
    dirs::data_dir()
        .map(|d| d.join("echovault").join("vault"))
        .unwrap_or_else(|| PathBuf::from("./vault"))
}

/// Lấy đường dẫn config directory mặc định (~/.config/echovault/)
pub fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("echovault"))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Lấy đường dẫn config file mặc định
pub fn default_config_path() -> PathBuf {
    default_config_dir().join("echovault.toml")
}

/// Lấy đường dẫn credentials file mặc định (trong config dir, không phải vault)
pub fn default_credentials_path() -> PathBuf {
    default_config_dir().join(".credentials.json")
}

#[allow(dead_code)]
impl Config {
    /// Tạo config mới với các giá trị mặc định
    pub fn new() -> Self {
        Self::default()
    }

    /// Tạo config với vault path cụ thể
    pub fn with_vault_path(vault_path: PathBuf) -> Self {
        Self {
            vault_path,
            ..Self::default()
        }
    }

    /// Load config từ file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Cannot parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Load config từ đường dẫn mặc định
    pub fn load_default() -> Result<Self> {
        let path = default_config_path();
        if path.exists() {
            Self::load(&path)
        } else {
            Ok(Self::default())
        }
    }

    /// Lưu config ra file
    pub fn save(&self, path: &Path) -> Result<()> {
        // Tạo thư mục cha nếu chưa tồn tại
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content =
            toml::to_string_pretty(self).with_context(|| "Cannot serialize config to TOML")?;

        std::fs::write(path, content)
            .with_context(|| format!("Cannot write config file: {}", path.display()))?;

        Ok(())
    }

    /// Lưu config ra đường dẫn mặc định
    pub fn save_default(&self) -> Result<PathBuf> {
        let path = default_config_path();
        self.save(&path)?;
        Ok(path)
    }

    /// Kiểm tra config đã được khởi tạo chưa (đã có remote)
    pub fn is_initialized(&self) -> bool {
        self.sync.remote.is_some()
    }

    /// Set remote repository URL
    pub fn set_remote(&mut self, remote: String) {
        self.sync.remote = Some(remote);
    }

    /// Lấy đường dẫn đến vault directory
    pub fn vault_dir(&self) -> &Path {
        &self.vault_path
    }

    /// Lấy đường dẫn đến index database
    pub fn index_db_path(&self) -> PathBuf {
        self.vault_path.join("index.db")
    }

    /// Lấy đường dẫn đến thư mục chứa encrypted files
    pub fn encrypted_dir(&self) -> PathBuf {
        self.vault_path.join("encrypted")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert!(!config.is_initialized());
        assert!(config.encryption.enabled);
    }

    #[test]
    fn test_save_and_load() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("test.toml");

        let mut config = Config::new();
        config.set_remote("https://github.com/user/vault.git".to_string());
        config.save(&config_path)?;

        let loaded = Config::load(&config_path)?;
        assert!(loaded.is_initialized());
        assert_eq!(
            loaded.sync.remote,
            Some("https://github.com/user/vault.git".to_string())
        );

        Ok(())
    }

    #[test]
    fn test_toml_serialization() -> Result<()> {
        let config = Config {
            version: 1,
            setup_complete: true,
            vault_path: PathBuf::from("/home/user/.echovault/vault"),
            sync: SyncConfig {
                remote: Some("https://github.com/user/vault.git".to_string()),
                repo_name: Some("user-vault".to_string()),
                provider: "github".to_string(),
            },
            encryption: EncryptionConfig { enabled: true },
            compression: CompressionConfig { enabled: true },
            extractors: ExtractorsConfig {
                enabled_sources: vec!["vscode-copilot".to_string()],
            },
        };

        let toml_str = toml::to_string_pretty(&config)?;
        assert!(toml_str.contains("version = 1"));
        assert!(toml_str.contains("vault_path ="));

        Ok(())
    }
}
