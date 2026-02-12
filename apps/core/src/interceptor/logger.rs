//! Conversation logger for intercepted API traffic.
//!
//! Logs request/response pairs as JSONL files, organized by domain and date.
//! Each exchange is stored as a single line in JSONL format for easy parsing.

use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// A single intercepted API exchange (request + response).
#[derive(Debug, Serialize, Deserialize)]
pub struct InterceptedExchange {
    pub timestamp: String,
    pub method: String,
    pub url: String,
    pub request_content_type: Option<String>,
    pub request_body: Option<serde_json::Value>,
    pub response_status: u16,
    pub response_content_type: String,
    pub response_body: Option<serde_json::Value>,
}

/// Pending request waiting for its response.
struct PendingRequest {
    method: String,
    url: String,
    content_type: Option<String>,
    body: Option<serde_json::Value>,
}

/// Logs intercepted conversations to disk.
pub struct ConversationLogger {
    output_dir: PathBuf,
    pending: Mutex<Option<PendingRequest>>,
}

impl ConversationLogger {
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            pending: Mutex::new(None),
        }
    }

    /// Check if there's a pending request waiting for response.
    pub fn has_pending(&self) -> bool {
        self.pending.lock().unwrap().is_some()
    }

    /// Log an intercepted request. Stores it as pending until response arrives.
    pub fn log_request(
        &self,
        method: &str,
        url: &str,
        headers: &hudsucker::hyper::HeaderMap,
        body: &[u8],
    ) {
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let body_json = if body.is_empty() {
            None
        } else {
            // Try to parse as JSON, fall back to base64 string
            serde_json::from_slice(body).ok().or_else(|| {
                Some(serde_json::Value::String(format!(
                    "[binary {} bytes]",
                    body.len()
                )))
            })
        };

        let pending = PendingRequest {
            method: method.to_string(),
            url: url.to_string(),
            content_type,
            body: body_json,
        };

        *self.pending.lock().unwrap() = Some(pending);
    }

    /// Log an intercepted response. Pairs with pending request and writes to disk.
    pub fn log_response(&self, status: u16, content_type: &str, body: &[u8]) {
        let pending = self.pending.lock().unwrap().take();
        let Some(req) = pending else {
            tracing::warn!("[interceptor] Response without pending request");
            return;
        };

        let response_body = if body.is_empty() {
            None
        } else {
            serde_json::from_slice(body).ok().or_else(|| {
                Some(serde_json::Value::String(format!(
                    "[binary {} bytes]",
                    body.len()
                )))
            })
        };

        let exchange = InterceptedExchange {
            timestamp: Utc::now().to_rfc3339(),
            method: req.method,
            url: req.url.clone(),
            request_content_type: req.content_type,
            request_body: req.body,
            response_status: status,
            response_content_type: content_type.to_string(),
            response_body,
        };

        // Determine output path: output_dir/<domain>/<YYYY-MM-DD>/<timestamp>.jsonl
        let domain = extract_domain(&req.url);
        let date = Local::now().format("%Y-%m-%d").to_string();
        let dir = self.output_dir.join(&domain).join(&date);

        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::error!(
                "[interceptor] Failed to create dir {}: {}",
                dir.display(),
                e
            );
            return;
        }

        let filename = format!(
            "{}_{}.jsonl",
            Utc::now().format("%H%M%S_%3f"),
            short_hash(&req.url)
        );
        let path = dir.join(&filename);

        match serde_json::to_string(&exchange) {
            Ok(line) => {
                if let Err(e) = append_line(&path, &line) {
                    tracing::error!("[interceptor] Failed to write {}: {}", path.display(), e);
                } else {
                    tracing::info!(
                        "[interceptor] Logged {} {} -> {} to {}",
                        exchange.method,
                        domain,
                        status,
                        path.display()
                    );
                }
            }
            Err(e) => {
                tracing::error!("[interceptor] Failed to serialize exchange: {}", e);
            }
        }
    }
}

/// Extract domain from URL.
fn extract_domain(url: &str) -> String {
    url.split("//")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("unknown")
        .split(':')
        .next()
        .unwrap_or("unknown")
        .to_string()
}

/// Generate a short hash for deduplication.
fn short_hash(s: &str) -> String {
    // Simple FNV-1a inspired hash, 6 hex chars
    let mut hash: u32 = 0x811c_9dc5;
    for b in s.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{:06x}", hash & 0xFFFFFF)
}

/// Append a line to a file (create if not exists).
fn append_line(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain() {
        // Simple domain
        assert_eq!(extract_domain("https://example.com"), "example.com");

        // With path
        assert_eq!(extract_domain("https://example.com/foo/bar"), "example.com");

        // With port
        assert_eq!(
            extract_domain("https://example.com:8080/foo"),
            "example.com"
        );

        // Subdomain
        assert_eq!(extract_domain("https://api.example.com"), "api.example.com");

        // No scheme
        assert_eq!(extract_domain("example.com/foo"), "example.com");

        // IP address
        assert_eq!(extract_domain("http://192.168.1.1"), "192.168.1.1");

        // Weird but valid cases according to current implementation
        assert_eq!(extract_domain("example.com"), "example.com");
    }

    #[test]
    fn test_short_hash() {
        // Consistency
        let h1 = short_hash("test");
        let h2 = short_hash("test");
        assert_eq!(h1, h2);

        // Different inputs
        let h3 = short_hash("test1");
        assert_ne!(h1, h3);

        // Length
        assert_eq!(h1.len(), 6);

        // Known value (regression test)
        assert_eq!(short_hash("test"), "d071e5");
    }
}
