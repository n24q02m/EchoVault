//! Vault management - Metadata và operations cho vault.
//!
//! Module này quản lý vault metadata, verification, và operations.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Vault metadata được lưu trong vault.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultMetadata {
    /// Version của metadata format
    pub version: u32,
    /// Vault có được mã hóa không
    pub encrypted: bool,
    /// Vault có được nén không
    pub compressed: bool,
    /// Salt cho key derivation (base64 encoded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salt: Option<String>,
    /// Thời gian tạo vault
    pub created_at: String,
}

impl VaultMetadata {
    /// Tạo metadata mới cho vault
    pub fn new(encrypted: bool, compressed: bool) -> Self {
        use chrono::Utc;
        use rand::RngCore;

        let salt = if encrypted {
            // Tạo salt random 32 bytes
            let mut salt_bytes = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut salt_bytes);
            Some(base64_encode(&salt_bytes))
        } else {
            None
        };

        Self {
            version: 1,
            encrypted,
            compressed,
            salt,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    /// Đọc metadata từ file
    pub fn load(vault_dir: &Path) -> Result<Self> {
        let path = vault_dir.join("vault.json");
        let content = fs::read_to_string(&path)
            .context(format!("Failed to read vault.json from {:?}", path))?;
        let metadata: Self =
            serde_json::from_str(&content).context("Failed to parse vault.json")?;
        Ok(metadata)
    }

    /// Lưu metadata vào file
    pub fn save(&self, vault_dir: &Path) -> Result<()> {
        let path = vault_dir.join("vault.json");
        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize vault metadata")?;
        fs::write(&path, content).context(format!("Failed to write vault.json to {:?}", path))?;
        Ok(())
    }

    /// Check xem vault.json có tồn tại không
    pub fn exists(vault_dir: &Path) -> bool {
        vault_dir.join("vault.json").exists()
    }

    /// Lấy salt bytes (decoded from base64)
    pub fn salt_bytes(&self) -> Result<Option<Vec<u8>>> {
        match &self.salt {
            Some(s) => {
                let bytes = base64_decode(s)?;
                Ok(Some(bytes))
            }
            None => Ok(None),
        }
    }
}

/// Verify passphrase bằng cách decrypt test data
pub fn verify_passphrase(vault_dir: &Path, passphrase: &str) -> Result<bool> {
    use crate::crypto::encryption::Encryptor;
    use crate::crypto::key_derivation::{derive_key, SALT_LEN};

    let metadata = VaultMetadata::load(vault_dir)?;

    if !metadata.encrypted {
        // Không encrypted, không cần verify
        return Ok(true);
    }

    let salt_bytes = metadata
        .salt_bytes()?
        .ok_or_else(|| anyhow::anyhow!("Vault is encrypted but salt is missing"))?;

    // Convert salt to fixed-size array
    if salt_bytes.len() != SALT_LEN {
        anyhow::bail!(
            "Invalid salt length: expected {}, got {}",
            SALT_LEN,
            salt_bytes.len()
        );
    }
    let mut salt_arr = [0u8; SALT_LEN];
    salt_arr.copy_from_slice(&salt_bytes);

    // Derive key từ passphrase và salt
    let key = derive_key(passphrase, &salt_arr)?;
    let encryptor = Encryptor::new(&key);

    // Tìm 1 file encrypted để test
    let test_file = find_encrypted_file(vault_dir)?;

    match test_file {
        Some(path) => {
            // Đọc và thử decrypt
            match encryptor.decrypt_file_to_bytes(&path) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false), // Wrong passphrase
            }
        }
        None => {
            // Không có file nào để test, tạm accept
            Ok(true)
        }
    }
}

/// Tìm 1 file encrypted trong vault để test
fn find_encrypted_file(vault_dir: &Path) -> Result<Option<std::path::PathBuf>> {
    // Tìm trong thư mục sessions/
    let sessions_dir = vault_dir.join("sessions");
    if sessions_dir.exists() {
        for entry in fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "enc") {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

// Base64 helpers
fn base64_encode(data: &[u8]) -> String {
    use std::io::Write;
    let mut buf = Vec::new();
    {
        let mut encoder = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::none());
        let _ = encoder.write_all(data);
    }
    // Simple base64 without external crate
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b = match chunk.len() {
            1 => [chunk[0], 0, 0],
            2 => [chunk[0], chunk[1], 0],
            _ => [chunk[0], chunk[1], chunk[2]],
        };
        result.push(CHARS[(b[0] >> 2) as usize] as char);
        result.push(CHARS[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(b[2] & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(data: &str) -> Result<Vec<u8>> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let chars: Vec<u8> = data.bytes().filter(|&b| b != b'=').collect();

    for chunk in chars.chunks(4) {
        if chunk.len() < 2 {
            break;
        }
        let b: Vec<u8> = chunk
            .iter()
            .map(|&c| CHARS.iter().position(|&x| x == c).unwrap_or(0) as u8)
            .collect();

        result.push((b[0] << 2) | (b.get(1).unwrap_or(&0) >> 4));
        if chunk.len() > 2 {
            result.push((b[1] << 4) | (b.get(2).unwrap_or(&0) >> 2));
        }
        if chunk.len() > 3 {
            result.push((b[2] << 6) | b.get(3).unwrap_or(&0));
        }
    }
    Ok(result)
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
        let metadata = VaultMetadata::new(true, true);
        assert!(metadata.encrypted);
        assert!(metadata.compressed);
        assert!(metadata.salt.is_some());

        // Save
        metadata.save(vault_dir).unwrap();
        assert!(VaultMetadata::exists(vault_dir));

        // Load
        let loaded = VaultMetadata::load(vault_dir).unwrap();
        assert_eq!(loaded.encrypted, metadata.encrypted);
        assert_eq!(loaded.compressed, metadata.compressed);
        assert_eq!(loaded.salt, metadata.salt);
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = b"Hello, World!";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }
}
