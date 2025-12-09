# Rclone Binaries

Folder này chứa rclone binaries cho mỗi platform.

## Download Instructions

Download rclone từ https://rclone.org/downloads/ và đặt vào folder này với tên file đúng format:

### Windows (x86_64)

```bash
# Download
curl -LO https://downloads.rclone.org/rclone-current-windows-amd64.zip
# Extract và rename
unzip rclone-current-windows-amd64.zip
mv rclone-*/rclone.exe rclone-x86_64-pc-windows-msvc.exe
```

### Linux (x86_64)

```bash
# Download
curl -LO https://downloads.rclone.org/rclone-current-linux-amd64.zip
# Extract và rename
unzip rclone-current-linux-amd64.zip
mv rclone-*/rclone rclone-x86_64-unknown-linux-gnu
chmod +x rclone-x86_64-unknown-linux-gnu
```

## File Naming Convention

Tauri sidecar yêu cầu tên file theo format: `{name}-{target_triple}{extension}`

| Platform | Target Triple              | Filename                              |
| -------- | -------------------------- | ------------------------------------- |
| Windows  | x86_64-pc-windows-msvc     | rclone-x86_64-pc-windows-msvc.exe     |
| Linux    | x86_64-unknown-linux-gnu   | rclone-x86_64-unknown-linux-gnu       |

## Notes

- Binaries KHÔNG được commit vào git (đã thêm vào .gitignore)
- CI/CD sẽ tự động download khi build
- Development: cần download thủ công hoặc sử dụng system rclone
