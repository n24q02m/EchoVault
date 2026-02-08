//! HTTP/HTTPS proxy using hudsucker for MITM interception.
//!
//! Intercepts traffic to configured domains, logging request/response pairs.
//! Traffic to non-target domains is tunneled through transparently.

use super::logger::ConversationLogger;
use super::InterceptorConfig;
use anyhow::Result;
use http_body_util::{BodyExt, Full};
use hudsucker::{
    certificate_authority::RcgenAuthority,
    hyper::{body::Bytes, Request, Response},
    rustls::crypto::aws_lc_rs,
    *,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::watch;

/// Consume a hudsucker Body and return the raw bytes.
async fn body_to_bytes(body: Body) -> Vec<u8> {
    match body.collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(_) => Vec::new(),
    }
}

/// Create a new Body from owned bytes.
fn body_from_vec(bytes: Vec<u8>) -> Body {
    Body::from(Full::new(Bytes::from(bytes)))
}

/// HTTP handler that intercepts and logs API traffic.
#[derive(Clone)]
struct InterceptHandler {
    target_domains: Arc<Vec<String>>,
    logger: Arc<ConversationLogger>,
}

impl InterceptHandler {
    fn is_target_host(&self, req: &Request<Body>) -> bool {
        let host = req
            .uri()
            .host()
            .or_else(|| {
                req.headers()
                    .get("host")
                    .and_then(|h| h.to_str().ok())
                    .map(|h| h.split(':').next().unwrap_or(h))
            })
            .unwrap_or("");

        self.target_domains
            .iter()
            .any(|d| host.ends_with(d.as_str()))
    }
}

impl HttpHandler for InterceptHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        if self.is_target_host(&req) {
            let method = req.method().to_string();
            let uri = req.uri().to_string();
            tracing::debug!("[interceptor] Capturing {} {}", method, uri);

            let (parts, body) = req.into_parts();
            let bytes = body_to_bytes(body).await;

            self.logger
                .log_request(&method, &uri, &parts.headers, &bytes);

            // Reconstruct request from raw bytes
            let req = Request::from_parts(parts, body_from_vec(bytes));
            req.into()
        } else {
            req.into()
        }
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        if self.logger.has_pending() {
            let (parts, body) = res.into_parts();
            let bytes = body_to_bytes(body).await;

            let status = parts.status.as_u16();
            let content_type = parts
                .headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .to_string();

            self.logger.log_response(status, &content_type, &bytes);

            Response::from_parts(parts, body_from_vec(bytes))
        } else {
            res
        }
    }
}

/// Run the proxy server. Blocks until shutdown signal received.
pub async fn run_proxy(
    config: InterceptorConfig,
    ca: RcgenAuthority,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    let logger = Arc::new(ConversationLogger::new(config.output_dir.clone()));
    let handler = InterceptHandler {
        target_domains: Arc::new(config.target_domains),
        logger,
    };

    tracing::info!("[interceptor] Starting proxy on {}", addr);

    let proxy = Proxy::builder()
        .with_addr(addr)
        .with_ca(ca)
        .with_rustls_connector(aws_lc_rs::default_provider())
        .with_http_handler(handler)
        .with_graceful_shutdown(async move {
            while !*shutdown_rx.borrow_and_update() {
                if shutdown_rx.changed().await.is_err() {
                    break;
                }
            }
        })
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build proxy: {}", e))?;

    proxy
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("Proxy error: {}", e))?;

    Ok(())
}
