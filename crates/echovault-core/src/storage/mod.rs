//! Storage module - Quản lý việc lưu trữ raw JSON files và index metadata.
//!
//! Module này chứa:
//! - SQLite index để tìm kiếm và lọc sessions nhanh chóng
//! - Chunked storage cho files lớn (vượt giới hạn GitHub)
//! - Các hàm tiện ích để quản lý vault directory

pub mod chunked;
pub mod index;

pub use chunked::compress_encrypt_chunk;
pub use index::SessionIndex;
