//! Utility để mở URL trong browser mặc định.
//!
//! Hỗ trợ cross-platform: Windows, macOS, Linux.
//! Chỉ hỗ trợ OS local (native), không hỗ trợ WSL hoặc remote environments.

use std::process::Command;

/// Mở URL trong browser mặc định.
///
/// Trả về `true` nếu mở thành công, `false` nếu thất bại.
/// Hỗ trợ:
/// - **Linux**: Sử dụng `xdg-open`
/// - **macOS**: Sử dụng `open`
/// - **Windows**: Sử dụng `cmd /c start`
///
/// # Arguments
///
/// * `url` - URL cần mở trong browser
///
/// # Returns
///
/// `true` nếu command được chạy thành công, `false` nếu thất bại
pub fn open_browser(url: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        // Windows native
        Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
            .is_ok()
    }

    #[cfg(target_os = "macos")]
    {
        // macOS
        Command::new("open").arg(url).spawn().is_ok()
    }

    #[cfg(target_os = "linux")]
    {
        // Linux native - dùng xdg-open
        Command::new("xdg-open").arg(url).spawn().is_ok()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        // Platform không được hỗ trợ
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_browser_doesnt_crash() {
        // Test function signature, không thực sự mở browser
        let _ = open_browser;
    }
}
