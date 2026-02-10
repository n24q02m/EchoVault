//! API Interceptor - MITM proxy to capture AI IDE API traffic.
//!
//! Captures HTTP/HTTPS traffic between AI IDEs and their API backends,
//! logging request/response pairs as clean JSONL files.
//!
//! Target domains:
//! - generativelanguage.googleapis.com (Gemini API)
//! - aiplatform.googleapis.com (Vertex AI)
//! - api.anthropic.com (Claude API)
//! - api.openai.com (OpenAI API)
//!
//! This module is feature-gated behind the `interceptor` feature.

#[cfg(feature = "interceptor")]
pub mod cert;
#[cfg(feature = "interceptor")]
pub mod logger;
#[cfg(feature = "interceptor")]
pub mod proxy;

#[cfg(feature = "interceptor")]
use anyhow::Result;
#[cfg(feature = "interceptor")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "interceptor")]
use std::path::PathBuf;
#[cfg(feature = "interceptor")]
use std::sync::Arc;
#[cfg(feature = "interceptor")]
use tokio::sync::watch;

/// Configuration for the interceptor proxy.
#[cfg(feature = "interceptor")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterceptorConfig {
    /// Port to listen on (default: 18080)
    pub port: u16,
    /// Domains to intercept (MITM). Others are tunneled through.
    pub target_domains: Vec<String>,
    /// Directory to store intercepted conversations
    pub output_dir: PathBuf,
    /// Directory to store CA certificate and key
    pub cert_dir: PathBuf,
}

#[cfg(feature = "interceptor")]
impl Default for InterceptorConfig {
    fn default() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("echovault");

        Self {
            port: 18080,
            target_domains: vec![
                "generativelanguage.googleapis.com".to_string(),
                "aiplatform.googleapis.com".to_string(),
                "api.anthropic.com".to_string(),
                "api.openai.com".to_string(),
            ],
            output_dir: data_dir.join("vault").join("intercepted"),
            cert_dir: data_dir.join("certs"),
        }
    }
}

/// State of the interceptor proxy.
#[cfg(feature = "interceptor")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterceptorState {
    Stopped,
    Running { port: u16 },
    Error(String),
}

/// Handle to a running interceptor proxy, used to stop it.
#[cfg(feature = "interceptor")]
pub struct InterceptorHandle {
    shutdown_tx: watch::Sender<bool>,
    state: Arc<std::sync::Mutex<InterceptorState>>,
}

#[cfg(feature = "interceptor")]
impl InterceptorHandle {
    pub fn state(&self) -> InterceptorState {
        self.state.lock().unwrap().clone()
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
        *self.state.lock().unwrap() = InterceptorState::Stopped;
        tracing::info!("[interceptor] Shutdown signal sent");
    }
}

/// Start the interceptor proxy. Returns a handle to stop it.
#[cfg(feature = "interceptor")]
pub async fn start(config: InterceptorConfig) -> Result<InterceptorHandle> {
    std::fs::create_dir_all(&config.output_dir)?;
    std::fs::create_dir_all(&config.cert_dir)?;

    let ca = cert::ensure_ca(&config.cert_dir)?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let state = Arc::new(std::sync::Mutex::new(InterceptorState::Stopped));

    let state_clone = state.clone();
    let port = config.port;

    tokio::spawn(async move {
        match proxy::run_proxy(config, ca.authority, shutdown_rx).await {
            Ok(()) => {
                tracing::info!("[interceptor] Proxy stopped cleanly");
                *state_clone.lock().unwrap() = InterceptorState::Stopped;
            }
            Err(e) => {
                tracing::error!("[interceptor] Proxy error: {}", e);
                *state_clone.lock().unwrap() = InterceptorState::Error(e.to_string());
            }
        }
    });

    *state.lock().unwrap() = InterceptorState::Running { port };

    Ok(InterceptorHandle { shutdown_tx, state })
}

/// Get proxy setup instructions for the current OS.
#[cfg(feature = "interceptor")]
pub fn proxy_setup_instructions(config: &InterceptorConfig) -> String {
    let port = config.port;
    let ca_cert_path = config.cert_dir.join("echovault-ca.crt");

    let mut s = String::new();
    s.push_str("# EchoVault Proxy Interceptor Setup\n\n");
    s.push_str(&format!("Proxy: http://127.0.0.1:{}\n", port));
    s.push_str(&format!("CA Cert: {}\n\n", ca_cert_path.display()));

    s.push_str("## 1. Trust the CA Certificate\n\n");
    #[cfg(target_os = "windows")]
    {
        s.push_str("```powershell\n");
        s.push_str(&format!(
            "Import-Certificate -FilePath \"{}\" -CertStoreLocation Cert:\\CurrentUser\\Root\n",
            ca_cert_path.display()
        ));
        s.push_str("```\n\n");
    }
    #[cfg(target_os = "linux")]
    {
        s.push_str("```bash\n");
        s.push_str(&format!(
            "sudo cp \"{}\" /usr/local/share/ca-certificates/echovault-ca.crt\n",
            ca_cert_path.display()
        ));
        s.push_str("sudo update-ca-certificates\n");
        s.push_str("```\n\n");
    }
    #[cfg(target_os = "macos")]
    {
        s.push_str("```bash\n");
        s.push_str(&format!(
            "sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain \"{}\"\n",
            ca_cert_path.display()
        ));
        s.push_str("```\n\n");
    }

    s.push_str("## 2. Set Environment Variables\n\n");
    s.push_str("```\n");
    s.push_str(&format!("HTTP_PROXY=http://127.0.0.1:{}\n", port));
    s.push_str(&format!("HTTPS_PROXY=http://127.0.0.1:{}\n", port));
    s.push_str("```\n");
    s
}
