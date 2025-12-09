//! Storage module - Quản lý việc lưu trữ raw JSON files và index metadata.
//!
//! Module này chứa:
//! - SQLite index để tìm kiếm và lọc sessions nhanh chóng
//! - Các hàm tiện ích để quản lý vault directory

pub mod index;

pub use index::SessionIndex;
