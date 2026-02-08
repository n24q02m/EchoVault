//! Embedding provider - OpenAI-compatible HTTP client for text embeddings.
//!
//! Supports any OpenAI-compatible embedding API:
//! - OpenAI (api.openai.com)
//! - Ollama (localhost:11434/v1)
//! - LiteLLM proxy
//! - vLLM, TGI, etc.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Status of an embedding provider check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProviderStatus {
    /// Provider is reachable and model works.
    Available { dimension: usize },
    /// Provider is reachable but the requested model was not found.
    ModelNotFound { message: String },
    /// Provider is unreachable.
    Unavailable { reason: String },
}

/// Embedding API provider using OpenAI-compatible HTTP endpoint.
pub struct EmbeddingProvider {
    api_base: String,
    api_key: Option<String>,
    model: String,
}

/// Request body for embedding API.
#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: Vec<&'a str>,
}

/// Response from embedding API.
#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    #[allow(dead_code)]
    model: Option<String>,
    #[allow(dead_code)]
    usage: Option<EmbeddingUsage>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    #[allow(dead_code)]
    index: usize,
}

#[derive(Deserialize)]
struct EmbeddingUsage {
    #[allow(dead_code)]
    prompt_tokens: Option<u64>,
    #[allow(dead_code)]
    total_tokens: Option<u64>,
}

/// Error response from embedding API.
#[allow(dead_code)]
#[derive(Deserialize)]
struct ErrorResponse {
    error: Option<ErrorDetail>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ErrorDetail {
    message: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
}

impl EmbeddingProvider {
    /// Create a new embedding provider.
    pub fn new(api_base: &str, api_key: Option<&str>, model: &str) -> Self {
        // Normalize API base URL (strip trailing slash)
        let api_base = api_base.trim_end_matches('/').to_string();

        Self {
            api_base,
            api_key: api_key.map(|s| s.to_string()),
            model: model.to_string(),
        }
    }

    /// Embed a single text and return its vector.
    pub fn embed_single(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.embed_batch(&[text])?;
        results
            .into_iter()
            .next()
            .context("Empty response from embedding API")
    }

    /// Embed multiple texts in a single API call.
    ///
    /// Returns vectors in the same order as input texts.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!("{}/embeddings", self.api_base);
        let body = EmbeddingRequest {
            model: &self.model,
            input: texts.to_vec(),
        };

        debug!(
            "Embedding {} texts via {} (model: {})",
            texts.len(),
            url,
            self.model
        );

        let mut request = ureq::post(&url).header("Content-Type", "application/json");

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", &format!("Bearer {}", key));
        }

        let mut response = request.send_json(&body).map_err(|e| match e {
            ureq::Error::StatusCode(status) => {
                // Try to parse error response body
                anyhow::anyhow!("Embedding API returned status {}", status)
            }
            ureq::Error::Io(io_err) => {
                anyhow::anyhow!("Embedding API connection failed: {}", io_err)
            }
            other => anyhow::anyhow!("Embedding API error: {}", other),
        })?;

        let resp: EmbeddingResponse = response
            .body_mut()
            .read_json()
            .context("Failed to parse embedding API response")?;

        // Sort by index to ensure correct order
        let mut data = resp.data;
        data.sort_by_key(|d| d.index);

        let vectors: Vec<Vec<f32>> = data.into_iter().map(|d| d.embedding).collect();

        if vectors.len() != texts.len() {
            warn!(
                "Embedding API returned {} vectors for {} inputs",
                vectors.len(),
                texts.len()
            );
        }

        Ok(vectors)
    }

    /// Check if the embedding API is reachable.
    pub fn health_check(&self) -> Result<bool> {
        // Try embedding a simple test string
        match self.embed_single("test") {
            Ok(v) => {
                debug!("Embedding API healthy, dimension={}", v.len());
                Ok(true)
            }
            Err(e) => {
                warn!("Embedding API health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Check provider status with detailed information.
    pub fn check_provider_status(&self) -> ProviderStatus {
        match self.embed_single("test") {
            Ok(v) => ProviderStatus::Available { dimension: v.len() },
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("not found") || msg.contains("model") {
                    ProviderStatus::ModelNotFound { message: msg }
                } else {
                    ProviderStatus::Unavailable { reason: msg }
                }
            }
        }
    }

    /// Get the model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the API base URL.
    pub fn api_base(&self) -> &str {
        &self.api_base
    }
}

/// Compute cosine similarity between two vectors.
///
/// Returns a value between -1.0 and 1.0, where 1.0 means identical direction.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator == 0.0 {
        return 0.0;
    }

    dot / denominator
}

/// Check if Ollama is running and reachable at the default endpoint.
///
/// Returns `Some(models)` with a list of available model names,
/// or `None` if Ollama is not reachable.
pub fn check_ollama_available() -> Option<Vec<String>> {
    let url = "http://localhost:11434/api/tags";
    let mut resp = ureq::get(url).call().ok()?;

    #[derive(Deserialize)]
    struct OllamaModels {
        models: Option<Vec<OllamaModel>>,
    }
    #[derive(Deserialize)]
    struct OllamaModel {
        name: String,
    }

    let models: OllamaModels = resp.body_mut().read_json().ok()?;
    Some(
        models
            .models
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.name)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let sim = cosine_similarity(&[], &[]);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched_dims() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
