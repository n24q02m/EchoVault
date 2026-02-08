//! Embedding module - Semantic embedding pipeline for conversations.
//!
//! Phase 4 of the EchoVault pipeline:
//! 1. Extractors copy raw files to vault (Phase 1)
//! 2. Parsers produce clean Markdown conversations (Phase 2)
//! 3. Interceptor captures API traffic (Phase 3)
//! 4. Embedding chunks + embeds conversations for semantic search (Phase 4)
//!
//! Supports any OpenAI-compatible embedding API (Ollama, LiteLLM, OpenAI, vLLM).
//! Vectors are stored in SQLite with cosine similarity search in Rust.

pub mod chunker;
pub mod provider;
pub mod store;

use crate::parsers::{all_parsers, parse_vault_source, ParsedConversation};
use anyhow::{Context, Result};
use chunker::{chunk_conversation, ChunkConfig};
use provider::EmbeddingProvider;
use serde::{Deserialize, Serialize};
use std::path::Path;
use store::EmbeddingStore;
use tracing::{debug, info, warn};

/// Configuration for the embedding pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// API base URL (e.g., "http://localhost:11434/v1" for Ollama)
    #[serde(default = "default_api_base")]
    pub api_base: String,

    /// Optional API key
    #[serde(default)]
    pub api_key: Option<String>,

    /// Model name (e.g., "nomic-embed-text", "text-embedding-3-small")
    #[serde(default = "default_model")]
    pub model: String,

    /// Target characters per chunk
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    /// Overlap characters between chunks
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,

    /// Number of texts per API batch call
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_api_base() -> String {
    "http://localhost:11434/v1".to_string()
}

fn default_model() -> String {
    "nomic-embed-text".to_string()
}

fn default_chunk_size() -> usize {
    1000
}

fn default_chunk_overlap() -> usize {
    200
}

fn default_batch_size() -> usize {
    32
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            api_base: default_api_base(),
            api_key: None,
            model: default_model(),
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            batch_size: default_batch_size(),
        }
    }
}

/// Result of embedding vault conversations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResult {
    /// Number of sessions processed
    pub sessions_processed: usize,
    /// Total chunks created
    pub chunks_created: usize,
    /// Sessions skipped (already embedded)
    pub sessions_skipped: usize,
    /// Errors encountered (session_id, error_message)
    pub errors: Vec<(String, String)>,
}

/// A semantic search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSearchResult {
    /// Session ID
    pub session_id: String,
    /// Source (vscode-copilot, cursor, etc.)
    pub source: String,
    /// Relevant chunk content
    pub chunk_content: String,
    /// Cosine similarity score (0.0 to 1.0)
    pub score: f32,
    /// Session title (if available from parsed data)
    pub title: Option<String>,
}

/// Embed all parsed conversations in the vault.
///
/// Reads parsed Markdown files, chunks them, calls the embedding API,
/// and stores vectors in `embeddings.db`.
///
/// Skips sessions that already have embeddings (incremental).
pub fn embed_vault(config: &EmbeddingConfig, vault_dir: &Path) -> Result<EmbedResult> {
    let sessions_dir = vault_dir.join("sessions");
    if !sessions_dir.exists() {
        return Ok(EmbedResult {
            sessions_processed: 0,
            chunks_created: 0,
            sessions_skipped: 0,
            errors: Vec::new(),
        });
    }

    // Open embedding store
    let store = EmbeddingStore::open(vault_dir).context("Failed to open embedding store")?;

    // Create embedding provider
    let provider =
        EmbeddingProvider::new(&config.api_base, config.api_key.as_deref(), &config.model);

    // Chunk config
    let chunk_config = ChunkConfig {
        chunk_size: config.chunk_size,
        chunk_overlap: config.chunk_overlap,
        min_chunk_size: 50,
    };

    // Collect all parsed conversations
    let parsers = all_parsers();
    let mut all_conversations: Vec<ParsedConversation> = Vec::new();

    for parser in &parsers {
        let (conversations, _errors) = parse_vault_source(parser.as_ref(), &sessions_dir);
        all_conversations.extend(conversations);
    }

    info!(
        "Embedding pipeline: {} conversations found",
        all_conversations.len()
    );

    let mut result = EmbedResult {
        sessions_processed: 0,
        chunks_created: 0,
        sessions_skipped: 0,
        errors: Vec::new(),
    };

    for conv in &all_conversations {
        // Skip empty conversations
        if conv.is_empty() {
            continue;
        }

        // Skip if already embedded
        match store.has_session(&conv.id) {
            Ok(true) => {
                result.sessions_skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                result.errors.push((conv.id.clone(), e.to_string()));
                continue;
            }
        }

        // Chunk the conversation
        let chunks = chunk_conversation(conv, &chunk_config);
        if chunks.is_empty() {
            debug!("Session {} produced no chunks, skipping", conv.id);
            continue;
        }

        // Embed chunks in batches
        let chunk_texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();
        let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(chunk_texts.len());

        for batch in chunk_texts.chunks(config.batch_size) {
            match provider.embed_batch(batch) {
                Ok(embeddings) => {
                    all_embeddings.extend(embeddings);
                }
                Err(e) => {
                    warn!("Failed to embed batch for session {}: {}", conv.id, e);
                    result.errors.push((conv.id.clone(), e.to_string()));
                    break;
                }
            }
        }

        // If we got all embeddings, store them
        if all_embeddings.len() == chunk_texts.len() {
            let chunk_pairs: Vec<(String, Vec<f32>)> = chunks
                .iter()
                .zip(all_embeddings.into_iter())
                .map(|(chunk, emb)| (chunk.content.clone(), emb))
                .collect();

            match store.store_session_chunks(&conv.id, &conv.source, &config.model, &chunk_pairs) {
                Ok(count) => {
                    result.sessions_processed += 1;
                    result.chunks_created += count;
                    debug!("Embedded session {}: {} chunks", conv.id, count);
                }
                Err(e) => {
                    result.errors.push((conv.id.clone(), e.to_string()));
                }
            }
        }
    }

    info!(
        "Embedding complete: {} processed, {} chunks, {} skipped, {} errors",
        result.sessions_processed,
        result.chunks_created,
        result.sessions_skipped,
        result.errors.len()
    );

    Ok(result)
}

/// Perform semantic search across all embedded conversations.
///
/// Uses hybrid search (vector + FTS5 keyword) when available,
/// with Reciprocal Rank Fusion (RRF) scoring.
/// Falls back to vector-only search if FTS5 query fails.
pub fn search_similar(
    config: &EmbeddingConfig,
    vault_dir: &Path,
    query: &str,
    limit: usize,
) -> Result<Vec<SemanticSearchResult>> {
    // Open embedding store
    let store = EmbeddingStore::open(vault_dir).context("Failed to open embedding store")?;

    // Create embedding provider and embed the query
    let provider =
        EmbeddingProvider::new(&config.api_base, config.api_key.as_deref(), &config.model);

    let query_embedding = provider
        .embed_single(query)
        .context("Failed to embed search query")?;

    // Try hybrid search first (vector + keyword), fall back to vector-only
    let alpha = 0.6; // Bias towards vector similarity
    let hybrid_results = store.search_hybrid_sessions(query, &query_embedding, limit, alpha);

    let parsed_dir = vault_dir.join("parsed");

    match hybrid_results {
        Ok(results) => {
            debug!("Hybrid search returned {} results", results.len());
            Ok(results
                .into_iter()
                .map(|r| {
                    let title = read_parsed_title(&parsed_dir, &r.source, &r.session_id);
                    SemanticSearchResult {
                        session_id: r.session_id,
                        source: r.source,
                        chunk_content: r.chunk_content,
                        score: r.score,
                        title,
                    }
                })
                .collect())
        }
        Err(e) => {
            // Fallback to vector-only search
            warn!("Hybrid search failed ({}), falling back to vector-only", e);
            let results = store.search_sessions(&query_embedding, limit)?;
            Ok(results
                .into_iter()
                .map(|r| {
                    let title = read_parsed_title(&parsed_dir, &r.source, &r.session_id);
                    SemanticSearchResult {
                        session_id: r.session_id,
                        source: r.source,
                        chunk_content: r.chunk_content,
                        score: r.score,
                        title,
                    }
                })
                .collect())
        }
    }
}

/// Get embedding store statistics.
pub fn get_stats(vault_dir: &Path) -> Result<store::StoreStats> {
    let store = EmbeddingStore::open(vault_dir)?;
    store.stats()
}

/// Try to read the title from a parsed Markdown file.
fn read_parsed_title(parsed_dir: &Path, source: &str, session_id: &str) -> Option<String> {
    let md_path = parsed_dir.join(source).join(format!("{}.md", session_id));
    if !md_path.exists() {
        return None;
    }

    // Read first few lines to find YAML frontmatter title
    let content = std::fs::read_to_string(&md_path).ok()?;
    for line in content.lines().take(10) {
        if let Some(title) = line.strip_prefix("title: ") {
            let title = title.trim().trim_matches('"');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }

    None
}
