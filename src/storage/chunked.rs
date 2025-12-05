//! Chunked storage với compression cho files lớn.
//!
//! Giải quyết giới hạn GitHub:
//! - 100MB per file (hard limit)
//! - 2GB per push (pack size)
//!
//! Workflow: JSON → gzip → encrypt → chunk (nếu cần)

use crate::crypto::Encryptor;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

/// Kích thước chunk tối đa (25MB - safe margin dưới 100MB GitHub limit)
const CHUNK_SIZE: usize = 25 * 1024 * 1024;

/// Kích thước file tối thiểu để chunking (50MB)
const CHUNK_THRESHOLD: usize = 50 * 1024 * 1024;

/// Extension cho file đã compress + encrypt
pub const COMPRESSED_EXT: &str = ".json.gz.enc";

/// Extension cho manifest
const MANIFEST_EXT: &str = ".manifest";

/// Manifest chứa metadata cho chunked files
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkManifest {
    /// Tên file gốc (không có path)
    original_file: String,
    /// Số lượng parts
    total_parts: usize,
    /// Kích thước mỗi chunk (trừ chunk cuối có thể nhỏ hơn)
    chunk_size: usize,
    /// Tổng kích thước sau compress + encrypt
    total_size: usize,
    /// Compression algorithm
    compression: String,
    /// Encryption algorithm
    encryption: String,
    /// Thông tin từng part
    parts: Vec<ChunkPart>,
}

/// Thông tin một chunk part
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkPart {
    /// Số thứ tự part (1-indexed)
    part: usize,
    /// Kích thước part này
    size: usize,
    /// SHA256 checksum
    sha256: String,
}

/// Compress data với gzip
fn compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    encoder.finish().context("Failed to compress data")
}

/// Decompress data từ gzip
fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .context("Failed to decompress data")?;
    Ok(decompressed)
}

/// Tính SHA256 checksum
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compress, encrypt và chunk một file JSON
///
/// # Arguments
/// * `source_path` - Đường dẫn file JSON nguồn
/// * `dest_dir` - Thư mục đích để lưu file encrypted
/// * `encryptor` - Encryptor để mã hóa
pub fn compress_encrypt_chunk(
    source_path: &Path,
    dest_dir: &Path,
    encryptor: &Encryptor,
) -> Result<()> {
    // Đọc file nguồn
    let data = fs::read(source_path).context("Cannot read source file")?;

    // Compress
    let compressed = compress(&data)?;

    // Encrypt
    let encrypted = encryptor.encrypt(&compressed)?;

    // Tên file gốc (không có extension)
    let file_stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Kiểm tra có cần chunk không
    if encrypted.len() < CHUNK_THRESHOLD {
        // File nhỏ - không cần chunk
        let dest_path = dest_dir.join(format!("{}{}", file_stem, COMPRESSED_EXT));
        fs::write(&dest_path, &encrypted)?;
        return Ok(());
    }

    // File lớn - cần chunk
    let chunks: Vec<&[u8]> = encrypted.chunks(CHUNK_SIZE).collect();
    let total_parts = chunks.len();
    let mut parts = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let part_num = i + 1;
        let part_path = dest_dir.join(format!("{}{}{:03}", file_stem, COMPRESSED_EXT, part_num));

        fs::write(&part_path, chunk)?;

        parts.push(ChunkPart {
            part: part_num,
            size: chunk.len(),
            sha256: sha256_hex(chunk),
        });
    }

    // Tạo manifest
    let manifest = ChunkManifest {
        original_file: format!("{}.json", file_stem),
        total_parts,
        chunk_size: CHUNK_SIZE,
        total_size: encrypted.len(),
        compression: "gzip".to_string(),
        encryption: "aes-256-gcm".to_string(),
        parts,
    };

    // Lưu manifest (không encrypt, để có thể đọc metadata)
    let manifest_path = dest_dir.join(format!("{}{}{}", file_stem, COMPRESSED_EXT, MANIFEST_EXT));
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, &manifest_json)?;

    Ok(())
}

/// Đọc file đơn (không chunk) đã compress + encrypt
#[allow(dead_code)]
fn read_single_file(path: &Path, encryptor: &Encryptor) -> Result<Vec<u8>> {
    let encrypted = fs::read(path)?;
    let compressed = encryptor.decrypt(&encrypted)?;
    decompress(&compressed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::key_derivation::derive_key_new;
    use tempfile::TempDir;

    #[test]
    fn test_compress_decompress() {
        let data = b"Hello, World! This is a test string that should compress well.";
        let compressed = compress(data).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_sha256_hex() {
        let data = b"test";
        let hash = sha256_hex(data);
        assert_eq!(hash.len(), 64); // SHA256 produces 64 hex chars
    }

    #[test]
    fn test_compress_encrypt_small_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let source_path = temp_dir.path().join("test.json");
        let dest_dir = temp_dir.path().join("encrypted");
        fs::create_dir_all(&dest_dir)?;

        // Tạo file test nhỏ
        let data = r#"{"message": "Hello, World!"}"#;
        fs::write(&source_path, data)?;

        // Encrypt
        let (key, _salt) = derive_key_new("test_password")?;
        let encryptor = Encryptor::new(&key);
        let result = compress_encrypt_chunk(&source_path, &dest_dir, &encryptor)?;

        // Verify
        assert!(!result.is_chunked);
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].exists());

        // Decrypt và verify
        let decrypted = read_single_file(&result.files[0], &encryptor)?;
        assert_eq!(data.as_bytes(), decrypted.as_slice());

        Ok(())
    }
}
