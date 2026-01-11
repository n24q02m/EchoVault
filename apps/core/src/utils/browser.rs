//! Utility to open URL in default browser.
//!
//! Supports cross-platform: Windows, macOS, Linux.
//! Only supports local OS (native), not WSL or remote environments.

use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows flag to prevent console window from appearing
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Open URL in default browser.
///
/// Returns `true` if opened successfully, `false` if failed.
/// Supports:
/// - **Linux**: Uses `xdg-open`
/// - **macOS**: Uses `open`
/// - **Windows**: Uses `cmd /c start`
///
/// # Arguments
///
/// * `url` - URL to open in browser
///
/// # Returns
///
/// `true` if command was executed successfully, `false` if failed
pub fn open_browser(url: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        // Windows native - use CREATE_NO_WINDOW to hide terminal
        let mut cmd = Command::new("cmd");
        cmd.args(["/c", "start", "", url]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn().is_ok()
    }

    #[cfg(target_os = "macos")]
    {
        // macOS
        Command::new("open").arg(url).spawn().is_ok()
    }

    #[cfg(target_os = "linux")]
    {
        // Linux native - use xdg-open
        Command::new("xdg-open").arg(url).spawn().is_ok()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        // Unsupported platform
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_browser_doesnt_crash() {
        // Test function signature, don't actually open browser
        let _ = open_browser;
    }
}
