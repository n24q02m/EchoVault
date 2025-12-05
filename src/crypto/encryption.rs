//! AES-256-GCM Encryption/Decryption.
//!
//! AES-GCM là Authenticated Encryption with Associated Data (AEAD),
//! cung cấp cả confidentiality và integrity cho dữ liệu.
//!
//! Đặc điểm:
//! - Military-grade encryption (AES-256)
//! - Hardware accelerated trên CPU hỗ trợ AES-NI
//! - Nonce 96-bit (12 bytes) - PHẢI unique cho mỗi message với cùng key
//! - Tag 128-bit (16 bytes) cho authentication

use super::key_derivation::KEY_LEN;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use rand::{rngs::OsRng, RngCore};
use std::path::Path;

/// Độ dài nonce (bytes) - 96 bits
pub const NONCE_LEN: usize = 12;

/// Độ dài authentication tag (bytes) - 128 bits
pub const TAG_LEN: usize = 16;

/// Header của encrypted file
/// Format: ECHOVAULT_V1 (12 bytes) + nonce (12 bytes) = 24 bytes
const MAGIC_HEADER: &[u8; 12] = b"ECHOVAULT_V1";

/// Struct để encrypt/decrypt với key đã derive
pub struct Encryptor {
    cipher: Aes256Gcm,
}

#[allow(dead_code)]
impl Encryptor {
    /// Tạo encryptor mới từ key (32 bytes)
    pub fn new(key: &[u8; KEY_LEN]) -> Self {
        let cipher = Aes256Gcm::new_from_slice(key).expect("Invalid key length");
        Self { cipher }
    }

    /// Encrypt data với random nonce
    /// Returns: nonce (12 bytes) || ciphertext (original + 16 bytes tag)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        // Combine: nonce || ciphertext
        let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt data
    /// Input format: nonce (12 bytes) || ciphertext (includes 16 bytes tag)
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < NONCE_LEN + TAG_LEN {
            bail!("Encrypted data too short");
        }

        // Extract nonce và ciphertext
        let nonce = Nonce::from_slice(&encrypted[..NONCE_LEN]);
        let ciphertext = &encrypted[NONCE_LEN..];

        // Decrypt
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

        Ok(plaintext)
    }

    /// Encrypt file và ghi ra file mới với extension .enc
    pub fn encrypt_file(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        // Đọc file gốc
        let plaintext = std::fs::read(input_path)
            .with_context(|| format!("Cannot read file: {}", input_path.display()))?;

        // Encrypt
        let encrypted = self.encrypt(&plaintext)?;

        // Ghi file với magic header
        let mut output = Vec::with_capacity(MAGIC_HEADER.len() + encrypted.len());
        output.extend_from_slice(MAGIC_HEADER);
        output.extend_from_slice(&encrypted);

        // Tạo thư mục cha nếu cần
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(output_path, output)
            .with_context(|| format!("Cannot write encrypted file: {}", output_path.display()))?;

        Ok(())
    }

    /// Decrypt file
    pub fn decrypt_file(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        // Đọc file encrypted
        let encrypted = std::fs::read(input_path)
            .with_context(|| format!("Cannot read encrypted file: {}", input_path.display()))?;

        // Verify magic header
        if encrypted.len() < MAGIC_HEADER.len() + NONCE_LEN + TAG_LEN {
            bail!("Invalid encrypted file: too short");
        }

        if &encrypted[..MAGIC_HEADER.len()] != MAGIC_HEADER {
            bail!("Invalid encrypted file: wrong magic header");
        }

        // Decrypt (bỏ qua magic header)
        let plaintext = self.decrypt(&encrypted[MAGIC_HEADER.len()..])?;

        // Ghi file
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(output_path, plaintext)
            .with_context(|| format!("Cannot write decrypted file: {}", output_path.display()))?;

        Ok(())
    }

    /// Decrypt file và trả về bytes (không ghi ra disk)
    pub fn decrypt_file_to_bytes(&self, input_path: &Path) -> Result<Vec<u8>> {
        let encrypted = std::fs::read(input_path)
            .with_context(|| format!("Cannot read encrypted file: {}", input_path.display()))?;

        if encrypted.len() < MAGIC_HEADER.len() + NONCE_LEN + TAG_LEN {
            bail!("Invalid encrypted file: too short");
        }

        if &encrypted[..MAGIC_HEADER.len()] != MAGIC_HEADER {
            bail!("Invalid encrypted file: wrong magic header");
        }

        self.decrypt(&encrypted[MAGIC_HEADER.len()..])
    }
}

/// Convenience function để encrypt file
#[allow(dead_code)]
pub fn encrypt_file(key: &[u8; KEY_LEN], input_path: &Path, output_path: &Path) -> Result<()> {
    let encryptor = Encryptor::new(key);
    encryptor.encrypt_file(input_path, output_path)
}

/// Convenience function để decrypt file
#[allow(dead_code)]
pub fn decrypt_file(key: &[u8; KEY_LEN], input_path: &Path, output_path: &Path) -> Result<()> {
    let encryptor = Encryptor::new(key);
    encryptor.decrypt_file(input_path, output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_key() -> [u8; KEY_LEN] {
        [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
            0x1c, 0x1d, 0x1e, 0x1f,
        ]
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() -> Result<()> {
        let encryptor = Encryptor::new(&test_key());
        let plaintext = b"Hello, EchoVault! This is a test message.";

        let encrypted = encryptor.encrypt(plaintext)?;
        let decrypted = encryptor.decrypt(&encrypted)?;

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
        Ok(())
    }

    #[test]
    fn test_encrypted_size() -> Result<()> {
        let encryptor = Encryptor::new(&test_key());
        let plaintext = b"test";

        let encrypted = encryptor.encrypt(plaintext)?;

        // Encrypted size = nonce (12) + plaintext + tag (16)
        assert_eq!(encrypted.len(), NONCE_LEN + plaintext.len() + TAG_LEN);
        Ok(())
    }

    #[test]
    fn test_different_nonce_each_time() -> Result<()> {
        let encryptor = Encryptor::new(&test_key());
        let plaintext = b"same message";

        let encrypted1 = encryptor.encrypt(plaintext)?;
        let encrypted2 = encryptor.encrypt(plaintext)?;

        // Cùng plaintext nhưng khác nonce -> khác ciphertext
        assert_ne!(encrypted1, encrypted2);
        Ok(())
    }

    #[test]
    fn test_wrong_key_fails() -> Result<()> {
        let encryptor1 = Encryptor::new(&test_key());
        let encryptor2 = Encryptor::new(&[1u8; KEY_LEN]); // Different key

        let plaintext = b"secret message";
        let encrypted = encryptor1.encrypt(plaintext)?;

        // Decrypt với key khác phải fail
        let result = encryptor2.decrypt(&encrypted);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_tampered_data_fails() -> Result<()> {
        let encryptor = Encryptor::new(&test_key());
        let plaintext = b"secret message";

        let mut encrypted = encryptor.encrypt(plaintext)?;

        // Tamper with ciphertext
        if let Some(byte) = encrypted.last_mut() {
            *byte ^= 0xFF;
        }

        // Decrypt phải fail do integrity check
        let result = encryptor.decrypt(&encrypted);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_file_encryption() -> Result<()> {
        let temp_dir = TempDir::new()?;

        let input_path = temp_dir.path().join("test.json");
        let encrypted_path = temp_dir.path().join("test.json.enc");
        let decrypted_path = temp_dir.path().join("test_decrypted.json");

        // Tạo file test
        let original_content = r#"{"message": "Hello, World!"}"#;
        std::fs::write(&input_path, original_content)?;

        // Encrypt
        let encryptor = Encryptor::new(&test_key());
        encryptor.encrypt_file(&input_path, &encrypted_path)?;

        // Verify encrypted file có magic header
        let encrypted = std::fs::read(&encrypted_path)?;
        assert_eq!(&encrypted[..12], MAGIC_HEADER);

        // Decrypt
        encryptor.decrypt_file(&encrypted_path, &decrypted_path)?;

        // Verify content
        let decrypted_content = std::fs::read_to_string(&decrypted_path)?;
        assert_eq!(decrypted_content, original_content);

        Ok(())
    }

    #[test]
    fn test_decrypt_to_bytes() -> Result<()> {
        let temp_dir = TempDir::new()?;

        let input_path = temp_dir.path().join("test.json");
        let encrypted_path = temp_dir.path().join("test.json.enc");

        // Tạo và encrypt file
        let original_content = b"test content";
        std::fs::write(&input_path, original_content)?;

        let encryptor = Encryptor::new(&test_key());
        encryptor.encrypt_file(&input_path, &encrypted_path)?;

        // Decrypt to bytes
        let decrypted = encryptor.decrypt_file_to_bytes(&encrypted_path)?;
        assert_eq!(decrypted, original_content);

        Ok(())
    }
}
