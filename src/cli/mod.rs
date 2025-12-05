//! CLI definitions và command implementations cho EchoVault.

pub mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// EchoVault - Black box for your AI conversations
#[derive(Parser)]
#[command(name = "echovault")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Khởi tạo vault mới với GitHub OAuth và encryption key
    Init {
        /// URL của remote repository (GitHub)
        #[arg(short, long)]
        remote: Option<String>,
    },

    /// Quét và liệt kê tất cả chat sessions có sẵn
    Scan {
        /// Nguồn để quét (mặc định: tất cả sources được hỗ trợ)
        #[arg(short, long)]
        source: Option<String>,
    },

    /// Trích xuất chat history vào vault (copy raw JSON)
    Extract {
        /// Nguồn để trích xuất (mặc định: tất cả)
        #[arg(short, long)]
        source: Option<String>,

        /// Thư mục output (mặc định: vault từ config)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Encrypt và đồng bộ vault lên GitHub
    Sync,
}
