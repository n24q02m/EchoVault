//! Command implementations cho EchoVault CLI.

use crate::extractors::{vscode_copilot::VSCodeCopilotExtractor, Extractor};
use crate::formatters::{markdown::MarkdownFormatter, Formatter};
use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

/// Quét tất cả sources để tìm chat sessions
pub fn scan(source: Option<String>) -> Result<()> {
    println!("{}", "Scanning for chat sessions...".cyan());

    // Hiện tại chỉ hỗ trợ VS Code Copilot
    let _source_filter = source.as_deref();

    let extractor = VSCodeCopilotExtractor::new();
    let workspaces = extractor.find_databases()?;

    if workspaces.is_empty() {
        println!("{}", "No VS Code Copilot chat sessions found.".yellow());
        return Ok(());
    }

    println!(
        "\n{} {} workspace(s) with chat sessions:\n",
        "Found".green(),
        workspaces.len().to_string().green().bold()
    );

    for (idx, workspace_path) in workspaces.iter().enumerate() {
        let workspace_name = extractor.get_workspace_name(workspace_path);

        // Đếm số sessions trong workspace
        let session_count = match extractor.count_sessions(workspace_path) {
            Ok(count) => count.to_string(),
            Err(_) => "?".to_string(),
        };

        println!(
            "  {}. {} [{}]",
            (idx + 1).to_string().cyan(),
            workspace_name.white().bold(),
            format!("{} sessions", session_count).dimmed()
        );
        println!("     {}", workspace_path.display().to_string().dimmed());
    }

    println!();
    Ok(())
}

/// Trích xuất chat history thành Markdown
pub fn extract(source: Option<String>, output: Option<PathBuf>) -> Result<()> {
    let output_dir = output.unwrap_or_else(|| PathBuf::from("./vault"));

    println!(
        "{} to {}",
        "Extracting chat history".cyan(),
        output_dir.display().to_string().yellow()
    );

    // Hiện tại chỉ hỗ trợ VS Code Copilot
    let _source_filter = source.as_deref();

    let extractor = VSCodeCopilotExtractor::new();
    let formatter = MarkdownFormatter::new();

    let workspaces = extractor.find_databases()?;

    if workspaces.is_empty() {
        println!("{}", "No VS Code Copilot chat sessions found.".yellow());
        return Ok(());
    }

    // Tạo output directory
    std::fs::create_dir_all(&output_dir)?;

    let mut total_sessions = 0;
    let mut total_files = 0;

    for workspace_path in &workspaces {
        let workspace_name = extractor.get_workspace_name(workspace_path);

        println!(
            "\n{} {} ({})",
            "Processing:".cyan(),
            workspace_name.white().bold(),
            workspace_path.display().to_string().dimmed()
        );

        // Extract sessions
        match extractor.extract_sessions(workspace_path) {
            Ok(sessions) => {
                for session in &sessions {
                    // Format thành Markdown
                    let markdown = formatter.format(session, &workspace_name)?;

                    // Tạo filename từ session
                    let filename = formatter.generate_filename(session);
                    let filepath = output_dir.join(&filename);

                    // Ghi file
                    std::fs::write(&filepath, markdown)?;

                    println!("  {} {}", "Created:".green(), filename);
                    total_files += 1;
                }
                total_sessions += sessions.len();
            }
            Err(e) => {
                println!("  {} {}", "Error:".red(), e);
            }
        }
    }

    println!(
        "\n{} Extracted {} sessions to {} files",
        "Done!".green().bold(),
        total_sessions.to_string().cyan(),
        total_files.to_string().cyan()
    );

    Ok(())
}
