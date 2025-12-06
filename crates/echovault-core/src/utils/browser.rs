//! Utility để mở URL trong browser mặc định.
//!
//! Hỗ trợ cross-platform: Windows, macOS, Linux và WSL.
//! Trên WSL, tự động phát hiện và sử dụng browser Windows.

use std::process::Command;

/// Kiểm tra xem đang chạy trong WSL hay không.
///
/// Đọc `/proc/version` để tìm chuỗi "microsoft" hoặc "WSL".
fn is_wsl() -> bool {
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        let version_lower = version.to_lowercase();
        return version_lower.contains("microsoft") || version_lower.contains("wsl");
    }
    false
}

/// Mở URL trong browser mặc định.
///
/// Trả về `true` nếu mở thành công, `false` nếu thất bại.
/// Hỗ trợ:
/// - **WSL**: Sử dụng `wslview` (từ wslu) hoặc `cmd.exe /c start`
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
        // Trên Windows native
        Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
            .is_ok()
    }

    #[cfg(target_os = "macos")]
    {
        // Trên macOS
        Command::new("open").arg(url).spawn().is_ok()
    }

    #[cfg(target_os = "linux")]
    {
        // Kiểm tra WSL trước
        if is_wsl() {
            // Thử wslview trước (từ wslu package, thường có sẵn trên WSL)
            if Command::new("wslview").arg(url).spawn().is_ok() {
                return true;
            }

            // Fallback: dùng cmd.exe trực tiếp
            // Set current_dir to C:\ để tránh UNC path warning
            if Command::new("cmd.exe")
                .current_dir("/mnt/c/")
                .args(["/c", "start", "", url])
                .stderr(std::process::Stdio::null())
                .spawn()
                .is_ok()
            {
                return true;
            }

            // Fallback cuối: dùng powershell.exe
            if Command::new("powershell.exe")
                .current_dir("/mnt/c/")
                .args(["-Command", &format!("Start-Process '{}'", url)])
                .stderr(std::process::Stdio::null())
                .spawn()
                .is_ok()
            {
                return true;
            }

            false
        } else {
            // Linux native - dùng xdg-open
            Command::new("xdg-open").arg(url).spawn().is_ok()
        }
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
    fn test_is_wsl_detection() {
        // Test này chỉ kiểm tra function không crash
        // Kết quả thực tế phụ thuộc vào môi trường
        let _ = is_wsl();
    }

    // Note: Không test open_browser trong unit test vì nó mở browser thật
}
