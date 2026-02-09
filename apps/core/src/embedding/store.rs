//! Embedding store - SQLite-based storage for text chunks and embedding vectors.
//!
//! Uses a dedicated `embeddings.db` in the vault directory.
//! Vectors are stored as f32 byte arrays (BLOB) for compact storage.
//! Cosine similarity search is performed in Rust for portability.

use crate::embedding::provider::cosine_similarity;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use tracing::{debug, info};

/// SQLite-based embedding storage.
pub struct EmbeddingStore {
    conn: Connection,
}

/// A search result from similarity search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Session ID this chunk belongs to
    pub session_id: String,
    /// Source (vscode-copilot, cursor, etc.)
    pub source: String,
    /// Chunk index within the session
    pub chunk_index: usize,
    /// The chunk text content
    pub chunk_content: String,
    /// Cosine similarity score (0.0 to 1.0)
    pub score: f32,
}

/// A hybrid search result combining vector + keyword scores.
#[derive(Debug, Clone)]
pub struct HybridResult {
    /// Session ID this chunk belongs to
    pub session_id: String,
    /// Source (vscode-copilot, cursor, etc.)
    pub source: String,
    /// Chunk index within the session
    pub chunk_index: usize,
    /// The chunk text content
    pub chunk_content: String,
    /// Fused score (higher is better)
    pub score: f32,
    /// Vector rank (None if not found in vector results)
    pub vector_rank: Option<usize>,
    /// Keyword rank (None if not found in keyword results)
    pub keyword_rank: Option<usize>,
}

/// Statistics about the embedding store.
#[derive(Debug, Clone)]
pub struct StoreStats {
    /// Total number of chunks stored
    pub total_chunks: usize,
    /// Number of unique sessions
    pub total_sessions: usize,
    /// Embedding dimension (0 if no embeddings yet)
    pub dimension: usize,
}

impl EmbeddingStore {
    /// Open or create the embedding store.
    pub fn open(vault_dir: &Path) -> Result<Self> {
        let db_path = vault_dir.join("embeddings.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("Cannot open embeddings database: {}", db_path.display()))?;

        // Performance pragmas
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -8000;",
        )?;

        let store = Self { conn };
        store.init_schema()?;

        Ok(store)
    }

    /// Open in-memory store for testing.
    #[allow(dead_code)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                source TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                embedding BLOB NOT NULL,
                model TEXT,
                dimension INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(session_id, chunk_index)
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_session
                ON chunks(session_id);

            CREATE INDEX IF NOT EXISTS idx_chunks_source
                ON chunks(source);

            -- Metadata table for store-level info
            CREATE TABLE IF NOT EXISTS embedding_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- FTS5 virtual table for keyword search on chunk content
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                session_id,
                source,
                content,
                content='chunks',
                content_rowid='rowid'
            );

            -- Auto-sync triggers for FTS5 index
            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO chunks_fts(rowid, session_id, source, content)
                VALUES (new.rowid, new.session_id, new.source, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, session_id, source, content)
                VALUES ('delete', old.rowid, old.session_id, old.source, old.content);
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, session_id, source, content)
                VALUES ('delete', old.rowid, old.session_id, old.source, old.content);
                INSERT INTO chunks_fts(rowid, session_id, source, content)
                VALUES (new.rowid, new.session_id, new.source, new.content);
            END;",
        )?;

        Ok(())
    }

    /// Store chunks with their embeddings for a session.
    ///
    /// Replaces any existing chunks for the same session.
    pub fn store_session_chunks(
        &self,
        session_id: &str,
        source: &str,
        model: &str,
        chunks: &[(String, Vec<f32>)],
    ) -> Result<usize> {
        if chunks.is_empty() {
            return Ok(0);
        }

        let dimension = chunks[0].1.len();
        let tx = self.conn.unchecked_transaction()?;

        // Delete existing chunks for this session
        tx.execute(
            "DELETE FROM chunks WHERE session_id = ?1",
            params![session_id],
        )?;

        let mut stmt = tx.prepare(
            "INSERT INTO chunks (session_id, source, chunk_index, content, embedding, model, dimension)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        for (i, (content, embedding)) in chunks.iter().enumerate() {
            let blob = embedding_to_blob(embedding);
            stmt.execute(params![
                session_id,
                source,
                i as i64,
                content,
                blob,
                model,
                dimension as i64,
            ])?;
        }

        drop(stmt);
        tx.commit()?;

        debug!(
            "Stored {} chunks for session {} (dim={})",
            chunks.len(),
            session_id,
            dimension
        );

        Ok(chunks.len())
    }

    /// Check if a session already has embeddings.
    pub fn has_session(&self, session_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM chunks WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Delete all chunks for a session.
    pub fn delete_session(&self, session_id: &str) -> Result<usize> {
        let affected = self.conn.execute(
            "DELETE FROM chunks WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(affected)
    }

    /// Search for similar chunks using cosine similarity.
    ///
    /// Loads all embeddings into memory and computes similarity.
    /// Efficient for up to ~100k chunks.
    pub fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, source, chunk_index, content, embedding
             FROM chunks",
        )?;

        let mut results: Vec<SearchResult> = stmt
            .query_map([], |row| {
                let session_id: String = row.get(0)?;
                let source: String = row.get(1)?;
                let chunk_index: i64 = row.get(2)?;
                let content: String = row.get(3)?;
                let blob: Vec<u8> = row.get(4)?;

                Ok((session_id, source, chunk_index as usize, content, blob))
            })?
            .filter_map(|r| r.ok())
            .map(|(session_id, source, chunk_index, content, blob)| {
                let embedding = blob_to_embedding(&blob);
                let score = cosine_similarity(query_embedding, &embedding);
                SearchResult {
                    session_id,
                    source,
                    chunk_index,
                    chunk_content: content,
                    score,
                }
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top-k
        results.truncate(limit);

        Ok(results)
    }

    /// Search similar chunks, grouped by session (best chunk per session).
    pub fn search_sessions(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let all_results = self.search_similar(query_embedding, limit * 3)?;

        // Keep only the best chunk per session
        let mut seen = std::collections::HashSet::new();
        let mut session_results = Vec::new();

        for result in all_results {
            if seen.insert(result.session_id.clone()) {
                session_results.push(result);
                if session_results.len() >= limit {
                    break;
                }
            }
        }

        Ok(session_results)
    }

    /// Keyword search using FTS5 on chunk content.
    ///
    /// Returns chunks matching the FTS5 query, ranked by BM25.
    pub fn search_keyword(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.session_id, c.source, c.chunk_index, c.content, rank
             FROM chunks c
             JOIN chunks_fts fts ON c.rowid = fts.rowid
             WHERE chunks_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let results: Vec<SearchResult> = stmt
            .query_map(params![query, limit as i64], |row| {
                let session_id: String = row.get(0)?;
                let source: String = row.get(1)?;
                let chunk_index: i64 = row.get(2)?;
                let content: String = row.get(3)?;
                let rank: f64 = row.get(4)?;

                Ok(SearchResult {
                    session_id,
                    source,
                    chunk_index: chunk_index as usize,
                    chunk_content: content,
                    // Convert BM25 rank to 0..1 score (rank is negative, closer to 0 is better)
                    score: (1.0 / (1.0 + rank.abs())) as f32,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Hybrid search combining vector similarity and keyword (FTS5) search.
    ///
    /// Uses Reciprocal Rank Fusion (RRF) to combine results:
    /// `score = alpha * 1/(k + rank_vector) + (1-alpha) * 1/(k + rank_keyword)`
    ///
    /// - `alpha` = 0.0..1.0 (0 = pure keyword, 1 = pure vector, default 0.6)
    /// - `k` = 60 (RRF constant)
    pub fn search_hybrid(
        &self,
        query: &str,
        query_embedding: &[f32],
        limit: usize,
        alpha: f32,
    ) -> Result<Vec<HybridResult>> {
        use std::collections::HashMap;

        let k = 60.0f32;
        let fetch_limit = limit * 3;

        // Get vector results
        let vector_results = self.search_similar(query_embedding, fetch_limit)?;

        // Get keyword results
        let keyword_results = self.search_keyword(query, fetch_limit)?;

        // Build fusion map: key = (session_id, chunk_index)
        let mut fusion: HashMap<(String, usize), HybridResult> = HashMap::new();

        // Add vector results with ranks
        for (rank, r) in vector_results.iter().enumerate() {
            let key = (r.session_id.clone(), r.chunk_index);
            let vector_score = alpha * (1.0 / (k + (rank + 1) as f32));
            let entry = fusion.entry(key).or_insert_with(|| HybridResult {
                session_id: r.session_id.clone(),
                source: r.source.clone(),
                chunk_index: r.chunk_index,
                chunk_content: r.chunk_content.clone(),
                score: 0.0,
                vector_rank: None,
                keyword_rank: None,
            });
            entry.score += vector_score;
            entry.vector_rank = Some(rank + 1);
        }

        // Add keyword results with ranks
        for (rank, r) in keyword_results.iter().enumerate() {
            let key = (r.session_id.clone(), r.chunk_index);
            let keyword_score = (1.0 - alpha) * (1.0 / (k + (rank + 1) as f32));
            let entry = fusion.entry(key).or_insert_with(|| HybridResult {
                session_id: r.session_id.clone(),
                source: r.source.clone(),
                chunk_index: r.chunk_index,
                chunk_content: r.chunk_content.clone(),
                score: 0.0,
                vector_rank: None,
                keyword_rank: None,
            });
            entry.score += keyword_score;
            entry.keyword_rank = Some(rank + 1);
        }

        // Sort by fused score descending
        let mut results: Vec<HybridResult> = fusion.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        Ok(results)
    }

    /// Hybrid search grouped by session (best chunk per session).
    pub fn search_hybrid_sessions(
        &self,
        query: &str,
        query_embedding: &[f32],
        limit: usize,
        alpha: f32,
    ) -> Result<Vec<HybridResult>> {
        let all_results = self.search_hybrid(query, query_embedding, limit * 3, alpha)?;

        let mut seen = std::collections::HashSet::new();
        let mut session_results = Vec::new();

        for result in all_results {
            if seen.insert(result.session_id.clone()) {
                session_results.push(result);
                if session_results.len() >= limit {
                    break;
                }
            }
        }

        Ok(session_results)
    }

    /// Get store statistics.
    pub fn stats(&self) -> Result<StoreStats> {
        let total_chunks: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;

        let total_sessions: i64 =
            self.conn
                .query_row("SELECT COUNT(DISTINCT session_id) FROM chunks", [], |row| {
                    row.get(0)
                })?;

        let dimension: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(dimension), 0) FROM chunks",
            [],
            |row| row.get(0),
        )?;

        Ok(StoreStats {
            total_chunks: total_chunks as usize,
            total_sessions: total_sessions as usize,
            dimension: dimension as usize,
        })
    }

    /// Get all session IDs that have embeddings.
    pub fn list_embedded_sessions(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT session_id FROM chunks ORDER BY session_id")?;

        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(ids)
    }

    /// Clear all embeddings.
    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM chunks", [])?;
        info!("Cleared all embeddings");
        Ok(())
    }
}

/// Convert f32 vector to byte blob for SQLite storage.
fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(embedding.len() * 4);
    for &val in embedding {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

/// Convert byte blob back to f32 vector.
fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| {
            let bytes: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(bytes)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_roundtrip() {
        let original = vec![1.0f32, 2.5, -std::f32::consts::PI, 0.0, 100.0];
        let blob = embedding_to_blob(&original);
        let recovered = blob_to_embedding(&blob);
        assert_eq!(original.len(), recovered.len());
        for (a, b) in original.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 1e-7);
        }
    }

    #[test]
    fn test_store_and_search() -> Result<()> {
        let store = EmbeddingStore::open_in_memory()?;

        // Store some test chunks
        let chunks = vec![
            ("Hello world".to_string(), vec![1.0, 0.0, 0.0]),
            ("Goodbye world".to_string(), vec![0.0, 1.0, 0.0]),
        ];

        store.store_session_chunks("s1", "test", "test-model", &chunks)?;
        assert!(store.has_session("s1")?);

        // Search with a query similar to first chunk
        let results = store.search_similar(&[1.0, 0.0, 0.0], 10)?;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk_content, "Hello world");
        assert!((results[0].score - 1.0).abs() < 1e-6);

        Ok(())
    }

    #[test]
    fn test_replace_session() -> Result<()> {
        let store = EmbeddingStore::open_in_memory()?;

        let chunks1 = vec![("Old content".to_string(), vec![1.0, 0.0])];
        store.store_session_chunks("s1", "test", "m1", &chunks1)?;

        let chunks2 = vec![
            ("New content A".to_string(), vec![0.5, 0.5]),
            ("New content B".to_string(), vec![0.0, 1.0]),
        ];
        store.store_session_chunks("s1", "test", "m1", &chunks2)?;

        let stats = store.stats()?;
        assert_eq!(stats.total_chunks, 2); // Old was replaced
        assert_eq!(stats.total_sessions, 1);

        Ok(())
    }

    #[test]
    fn test_delete_session() -> Result<()> {
        let store = EmbeddingStore::open_in_memory()?;

        let chunks = vec![("Content".to_string(), vec![1.0])];
        store.store_session_chunks("s1", "test", "m1", &chunks)?;
        assert!(store.has_session("s1")?);

        store.delete_session("s1")?;
        assert!(!store.has_session("s1")?);

        Ok(())
    }

    #[test]
    fn test_search_sessions() -> Result<()> {
        let store = EmbeddingStore::open_in_memory()?;

        // Two sessions with multiple chunks each
        store.store_session_chunks(
            "s1",
            "src1",
            "m1",
            &[
                ("s1 chunk 0".to_string(), vec![1.0, 0.0, 0.0]),
                ("s1 chunk 1".to_string(), vec![0.9, 0.1, 0.0]),
            ],
        )?;

        store.store_session_chunks(
            "s2",
            "src2",
            "m1",
            &[
                ("s2 chunk 0".to_string(), vec![0.0, 1.0, 0.0]),
                ("s2 chunk 1".to_string(), vec![0.0, 0.0, 1.0]),
            ],
        )?;

        let results = store.search_sessions(&[1.0, 0.0, 0.0], 10)?;
        assert_eq!(results.len(), 2);
        // s1 should rank higher
        assert_eq!(results[0].session_id, "s1");

        Ok(())
    }
}
