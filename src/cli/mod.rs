//! CLI definitions và command implementations cho EchoVault.

pub mod commands;

use clap::{Parser, Subcommand};

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

    /// Extract, encrypt và đồng bộ vault lên GitHub (tự động setup nếu lần đầu)
    Sync {
        /// URL của remote repository (GitHub)
        #[arg(short, long)]
        remote: Option<String>,
    },
}
