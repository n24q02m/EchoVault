# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.x.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability, please **DO NOT** create a public issue.

Instead, please email: **quangminh2402.dev@gmail.com**

Include:

1. Detailed description of the vulnerability
2. Steps to reproduce
3. Potential impact
4. Suggested fix (if any)

You will receive acknowledgment within 48 hours.

## Data Privacy Notice

> [!CAUTION]
> EchoVault syncs your AI chat history to cloud storage. This data may contain:
>
> - Code snippets and file paths
> - API keys or secrets mentioned in conversations
> - Personal information

**Recommendations:**

1. Review chat history for sensitive data before enabling sync
2. Use a private cloud storage account
3. Encryption feature coming in v2.0

## Security Measures

- Rclone credentials stored via OS keyring
- Local SQLite database (not exposed)
- No telemetry or data collection
- Regular dependency updates via Dependabot
