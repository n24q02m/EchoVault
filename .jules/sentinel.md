## 2025-02-14 - Insecure Config File Permissions
**Vulnerability:** The configuration file (`echovault.toml`) stores sensitive API keys in plain text and was created with default file permissions (readable by group/others).
**Learning:** `std::fs::write` uses default umask/permissions, which are often too permissive for sensitive files.
**Prevention:** Explicitly restrict file permissions to `0o600` (read/write owner only) using `std::os::unix::fs::PermissionsExt` immediately after writing sensitive files.
