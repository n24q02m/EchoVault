//! EchoVault CLI - Black box for your AI conversations
//!
//! Trích xuất, đồng bộ và tìm kiếm lịch sử chat AI từ nhiều IDE.
//! Nguyên tắc: Lưu trữ raw JSON gốc, không format/transform data.

mod cli;
mod config;
mod crypto;
mod extractors;
mod storage;
mod sync;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { source } => {
            cli::commands::scan(source)?;
        }
        Commands::Sync { remote } => {
            cli::commands::sync_vault(remote)?;
        }
    }

    Ok(())
}
