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
    /// Quét và liệt kê tất cả chat sessions có sẵn
    Scan {
        /// Nguồn để quét (mặc định: tất cả sources được hỗ trợ)
        #[arg(short, long)]
        source: Option<String>,
    },

    /// Trích xuất chat history thành Markdown
    Extract {
        /// Nguồn để trích xuất (mặc định: tất cả)
        #[arg(short, long)]
        source: Option<String>,

        /// Thư mục output (mặc định: ./vault)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}
