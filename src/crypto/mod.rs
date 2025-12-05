//! Crypto module - Mã hóa và giải mã dữ liệu với AES-256-GCM.
//!
//! Module này chứa:
//! - AES-256-GCM encryption/decryption
//! - Argon2id key derivation từ passphrase
//! - Nonce generation và management

pub mod encryption;
pub mod key_derivation;

pub use encryption::Encryptor;
// Re-export các hằng số và functions cho Desktop App sử dụng sau này
#[allow(unused_imports)]
pub use key_derivation::{derive_key_new, KEY_LEN, SALT_LEN};
