//! Command implementations cho EchoVault CLI.
//!
//! Các commands chính:
//! - scan: Quét và liệt kê tất cả chat sessions có sẵn
//! - extract: Copy raw JSON files vào vault (KHÔNG format)

use crate::extractors::{vscode_copilot::VSCodeCopilotExtractor, Extractor};
use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

/// Quét tất cả sources để tìm chat sessions
pub fn scan(source: Option<String>) -> Result<()> {
    println!("{}", "Scanning for chat sessions...".cyan());

    // Hiện tại chỉ hỗ trợ VS Code Copilot
    let _source_filter = source.as_deref();

    let extractor = VSCodeCopilotExtractor::new();
    let locations = extractor.find_storage_locations()?;

    if locations.is_empty() {
        println!("{}", "No VS Code Copilot chat sessions found.".yellow());
        return Ok(());
    }

    println!(
        "\n{} {} workspace(s) with chat sessions:\n",
        "Found".green(),
        locations.len().to_string().green().bold()
    );

    for (idx, location) in locations.iter().enumerate() {
        let workspace_name = extractor.get_workspace_name(location);

        // Đếm số sessions
        let session_count = match extractor.count_sessions(location) {
            Ok(count) => count.to_string(),
            Err(_) => "?".to_string(),
        };

        println!(
            "  {}. {} [{}]",
            (idx + 1).to_string().cyan(),
            workspace_name.white().bold(),
            format!("{} sessions", session_count).dimmed()
        );
        println!("     {}", location.display().to_string().dimmed());
    }

    println!();
    Ok(())
}

/// Trích xuất chat history - CHỈ COPY raw JSON files, KHÔNG format
pub fn extract(source: Option<String>, output: Option<PathBuf>) -> Result<()> {
    let output_dir = output.unwrap_or_else(|| PathBuf::from("./vault"));

    println!(
        "{} to {}",
        "Extracting raw JSON files".cyan(),
        output_dir.display().to_string().yellow()
    );

    // Hiện tại chỉ hỗ trợ VS Code Copilot
    let _source_filter = source.as_deref();

    let extractor = VSCodeCopilotExtractor::new();
    let locations = extractor.find_storage_locations()?;

    if locations.is_empty() {
        println!("{}", "No VS Code Copilot chat sessions found.".yellow());
        return Ok(());
    }

    // Tạo output directory
    std::fs::create_dir_all(&output_dir)?;

    let mut total_sessions = 0;
    let mut total_files = 0;
    let mut index: Vec<crate::extractors::SessionMetadata> = Vec::new();

    for location in &locations {
        let workspace_name = extractor.get_workspace_name(location);

        println!(
            "\n{} {} ({})",
            "Processing:".cyan(),
            workspace_name.white().bold(),
            location.display().to_string().dimmed()
        );

        // List session files
        match extractor.list_session_files(location) {
            Ok(sessions) => {
                for session in &sessions {
                    // Copy raw file vào vault
                    match extractor.copy_to_vault(session, &output_dir) {
                        Ok(vault_path) => {
                            // Cập nhật metadata với vault_path
                            let mut metadata = session.metadata.clone();
                            metadata.vault_path = vault_path.clone();
                            index.push(metadata);

                            let filename = vault_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            println!("  {} {}", "Copied:".green(), filename);
                            total_files += 1;
                        }
                        Err(e) => {
                            println!(
                                "  {} {} - {}",
                                "Error:".red(),
                                session.session_id,
                                e
                            );
                        }
                    }
                }
                total_sessions += sessions.len();
            }
            Err(e) => {
                println!("  {} {}", "Error:".red(), e);
            }
        }
    }

    // Lưu index file
    let index_path = output_dir.join("index.json");
    let index_json = serde_json::to_string_pretty(&index)?;
    std::fs::write(&index_path, index_json)?;

    println!(
        "\n{} Copied {} sessions to {} raw JSON files",
        "Done!".green().bold(),
        total_sessions.to_string().cyan(),
        total_files.to_string().cyan()
    );
    println!(
        "Index saved to: {}",
        index_path.display().to_string().dimmed()
    );

    Ok(())
}
