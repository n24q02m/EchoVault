//! Key derivation với Argon2id.
//!
//! Argon2id là thuật toán key derivation được khuyến nghị cho passwords,
//! kết hợp ưu điểm của Argon2i (chống side-channel attacks) và
//! Argon2d (chống GPU cracking).

use anyhow::{anyhow, bail, Context, Result};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2, Params,
};
use rand::{rngs::OsRng, RngCore};

/// Độ dài salt (bytes)
pub const SALT_LEN: usize = 16;

/// Độ dài key (bytes) - 256 bits cho AES-256
pub const KEY_LEN: usize = 32;

/// Tham số Argon2id - cân bằng giữa security và performance
/// - Memory: 64 MiB (đủ chống GPU attacks)
/// - Iterations: 3 (khuyến nghị cho interactive logins)
/// - Parallelism: 4 (sử dụng nhiều CPU cores)
const ARGON2_MEMORY_KIB: u32 = 64 * 1024; // 64 MiB
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;

/// Derive encryption key từ passphrase sử dụng Argon2id.
///
/// # Arguments
/// * `passphrase` - Mật khẩu của người dùng
/// * `salt` - Salt 16 bytes (nên random và unique cho mỗi vault)
///
/// # Returns
/// * 32-byte encryption key cho AES-256
pub fn derive_key(passphrase: &str, salt: &[u8; SALT_LEN]) -> Result<[u8; KEY_LEN]> {
    // Tạo Argon2id hasher với custom params
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(KEY_LEN),
    )
    .map_err(|e| anyhow!("Invalid Argon2 parameters: {}", e))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    // Encode salt cho password_hash crate
    let salt_string = SaltString::encode_b64(salt)
        .map_err(|e| anyhow!("Cannot encode salt for Argon2: {}", e))?;

    // Hash password và extract raw key
    let password_hash = argon2
        .hash_password(passphrase.as_bytes(), &salt_string)
        .map_err(|e| anyhow!("Cannot derive key from passphrase: {}", e))?;

    // Extract hash output
    let hash_output = password_hash.hash.context("No hash output from Argon2")?;

    // Convert to fixed-size array
    let hash_bytes = hash_output.as_bytes();
    if hash_bytes.len() < KEY_LEN {
        bail!("Argon2 hash output too short");
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&hash_bytes[..KEY_LEN]);

    Ok(key)
}

/// Generate random salt cho key derivation
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Derive key với salt mới (cho lần init đầu tiên)
/// Trả về cả key và salt để lưu trữ
pub fn derive_key_new(passphrase: &str) -> Result<([u8; KEY_LEN], [u8; SALT_LEN])> {
    let salt = generate_salt();
    let key = derive_key(passphrase, &salt)?;
    Ok((key, salt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() -> Result<()> {
        let passphrase = "test_password_123";
        let salt = [0u8; SALT_LEN]; // Fixed salt for testing

        let key1 = derive_key(passphrase, &salt)?;
        let key2 = derive_key(passphrase, &salt)?;

        // Cùng passphrase + salt phải cho cùng key
        assert_eq!(key1, key2);

        Ok(())
    }

    #[test]
    fn test_derive_key_different_passphrase() -> Result<()> {
        let salt = [0u8; SALT_LEN];

        let key1 = derive_key("password1", &salt)?;
        let key2 = derive_key("password2", &salt)?;

        // Khác passphrase phải cho khác key
        assert_ne!(key1, key2);

        Ok(())
    }

    #[test]
    fn test_derive_key_different_salt() -> Result<()> {
        let passphrase = "same_password";
        let salt1 = [0u8; SALT_LEN];
        let salt2 = [1u8; SALT_LEN];

        let key1 = derive_key(passphrase, &salt1)?;
        let key2 = derive_key(passphrase, &salt2)?;

        // Khác salt phải cho khác key
        assert_ne!(key1, key2);

        Ok(())
    }

    #[test]
    fn test_key_length() -> Result<()> {
        let passphrase = "test";
        let salt = [0u8; SALT_LEN];

        let key = derive_key(passphrase, &salt)?;

        // Key phải đúng 32 bytes (256 bits)
        assert_eq!(key.len(), KEY_LEN);

        Ok(())
    }

    #[test]
    fn test_derive_key_new() -> Result<()> {
        let (key1, salt1) = derive_key_new("password")?;
        let (key2, salt2) = derive_key_new("password")?;

        // Mỗi lần derive_key_new phải tạo salt khác nhau
        assert_ne!(salt1, salt2);

        // Và do salt khác, key cũng phải khác
        assert_ne!(key1, key2);

        Ok(())
    }
}
