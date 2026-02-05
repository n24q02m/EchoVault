## 2024-12-19 - [CRITICAL] Disabled CSP in Tauri Config
**Vulnerability:** The Content Security Policy (CSP) was explicitly set to `null` in `tauri.conf.json`, allowing unrestricted script execution and resource loading (potential XSS vector).
**Learning:** Defaulting to `null` disables critical browser security mechanisms. Always start with a strict policy.
**Prevention:** Enforce a strict CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: data:;`.
