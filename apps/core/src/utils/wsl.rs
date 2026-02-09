//! WSL (Windows Subsystem for Linux) utilities.
//!
//! Provides cross-platform helpers to detect WSL distributions
//! and resolve home directories from Windows host.
//! Only active on Windows — all functions return empty results on other platforms.

use std::path::PathBuf;
use tracing::info;

/// Information about a WSL distribution.
#[derive(Debug, Clone)]
pub struct WslDistro {
    /// Distribution name (e.g., "Ubuntu", "Debian")
    pub name: String,
    /// UNC base path (e.g., \\wsl.localhost\Ubuntu)
    pub base_path: PathBuf,
}

/// Check if WSL is available on this system.
/// Returns false on non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub fn is_wsl_available() -> bool {
    false
}

/// Check if WSL is available on this system.
#[cfg(target_os = "windows")]
pub fn is_wsl_available() -> bool {
    // Prefer \\wsl.localhost\ (Win11+, more robust)
    // Fallback to \\wsl$\ (Win10 1903+)
    std::fs::read_dir(r"\\wsl.localhost").is_ok() || std::fs::read_dir(r"\\wsl$").is_ok()
}

/// List all available WSL distributions.
/// Returns empty vec on non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub fn list_wsl_distros() -> Vec<WslDistro> {
    Vec::new()
}

/// List all available WSL distributions.
#[cfg(target_os = "windows")]
pub fn list_wsl_distros() -> Vec<WslDistro> {
    let mut distros = Vec::new();

    // Try \\wsl.localhost\ first (Win11+, more robust, survives network changes)
    let wsl_roots = [r"\\wsl.localhost", r"\\wsl$"];

    for wsl_root in &wsl_roots {
        if let Ok(entries) = std::fs::read_dir(wsl_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        // Avoid duplicates if both \\wsl.localhost and \\wsl$ work
                        if !distros.iter().any(|d: &WslDistro| d.name == name) {
                            distros.push(WslDistro {
                                name: name.to_string(),
                                base_path: path,
                            });
                        }
                    }
                }
            }
            // If we found distros from \\wsl.localhost, no need to try \\wsl$
            if !distros.is_empty() {
                break;
            }
        }
    }

    if !distros.is_empty() {
        info!(
            "[WSL] Found {} distributions: {:?}",
            distros.len(),
            distros.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
    }

    distros
}

/// Resolve all home directories for a WSL distribution.
/// Scans /home/* within the WSL filesystem.
#[cfg(not(target_os = "windows"))]
pub fn resolve_wsl_homes(_distro: &WslDistro) -> Vec<PathBuf> {
    Vec::new()
}

/// Resolve all home directories for a WSL distribution.
#[cfg(target_os = "windows")]
pub fn resolve_wsl_homes(distro: &WslDistro) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    let home_dir = distro.base_path.join("home");

    if home_dir.exists() && home_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&home_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    homes.push(path);
                }
            }
        }
    }

    // Also check /root
    let root_home = distro.base_path.join("root");
    if root_home.exists() && root_home.is_dir() {
        homes.push(root_home);
    }

    homes
}

/// Scan WSL distributions and find paths matching a given relative subpath.
/// For example, `find_wsl_paths(".config/Code/User/workspaceStorage")` will scan
/// all WSL distros and return all matching paths.
///
/// Returns empty vec on non-Windows platforms.
pub fn find_wsl_paths(relative_subpath: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for distro in list_wsl_distros() {
        for home in resolve_wsl_homes(&distro) {
            let candidate = home.join(relative_subpath);
            if candidate.exists() {
                info!("[WSL] Found path in {}: {:?}", distro.name, candidate);
                paths.push(candidate);
            }
        }
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_wsl_available_returns_bool() {
        // Just ensure it doesn't panic
        let _ = is_wsl_available();
    }

    #[test]
    fn test_list_wsl_distros_returns_vec() {
        let distros = list_wsl_distros();
        // On non-Windows or without WSL, should be empty
        for d in &distros {
            assert!(!d.name.is_empty());
            assert!(!d.base_path.to_string_lossy().is_empty());
        }
    }

    #[test]
    fn test_find_wsl_paths_returns_vec() {
        let paths = find_wsl_paths(".config/Code/User/workspaceStorage");
        for p in &paths {
            assert!(p.exists());
        }
    }
}
