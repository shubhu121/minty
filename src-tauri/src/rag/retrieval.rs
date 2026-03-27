//! Hybrid retrieval engine: combines vector similarity (LanceDB) with
//! BM25 keyword scoring (SQLite FTS5) using Reciprocal Rank Fusion.
//!
//! RRF score = Σ 1 / (k + rank) across all result lists, where k = 60.

use std::collections::HashMap;

use arrow_array::{Array, RecordBatch, StringArray};
use fastembed::EmbeddingModel;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use serde::Serialize;
use sqlx::SqlitePool;

use super::embedding_model::{load_shared_text_embedding, SharedTextEmbedding};
use super::errors::SearchError;

/// The RRF constant. Higher values dampen the effect of rank differences.
const RRF_K: f32 = 60.0;

#[derive(Debug, Clone, Serialize)]
pub struct RetrievedChunk {
    pub chunk_id: String,
    pub note_id: String,
    pub note_title: String,
    pub text: String,
    pub heading_path: String,
    pub char_start: i32,
    pub char_end: i32,
    /// Combined RRF score (higher = better).
    pub rrf_score: f32,
    /// Raw vector similarity score (1 / (1 + L2 distance)).
    pub vector_score: f32,
    /// Raw BM25 score from FTS5 (higher = better keyword match).
    pub bm25_score: f32,
}

#[derive(Debug, Clone)]
struct VectorHit {
    chunk_id: String,
    note_id: String,
    text: String,
    heading_path: String,
    char_start: i32,
    char_end: i32,
    score: f32,
}

#[derive(Debug, Clone)]
struct Bm25Hit {
    chunk_id: String,
    note_id: String,
    text: String,
    heading_path: String,
    score: f64,
}

pub struct HybridRetriever {
    pub lance_conn: lancedb::Connection,
    model: SharedTextEmbedding,
}

impl HybridRetriever {
    /// Create a new hybrid retriever with its own fastembed model instance.
    pub async fn new(
        lance_db_path: std::path::PathBuf,
        model_cache_path: std::path::PathBuf,
    ) -> Result<Self, SearchError> {
        let lance_path_str = lance_db_path.to_string_lossy().to_string();
        let lance_conn = lancedb::connect(&lance_path_str)
            .execute()
            .await
            .map_err(|e| SearchError::VectorStore(e.to_string()))?;

        let model =
            load_shared_text_embedding(EmbeddingModel::MultilingualE5Small, Some(model_cache_path))
                .await
                .map_err(SearchError::Embedding)?;

        Ok(Self { lance_conn, model })
    }

    pub async fn retrieve(
        &self,
        query: &str,
        top_k: usize,
        db: &SqlitePool,
    ) -> Result<Vec<RetrievedChunk>, SearchError> {
        let fetch_count = top_k * 2; // over-fetch for better fusion

        // Run both searches in parallel
        let (vector_results, bm25_results) = tokio::join!(
            self.vector_search(query, fetch_count),
            bm25_search(query, fetch_count, db),
        );

        let vector_hits = vector_results.unwrap_or_else(|e| {
            eprintln!("[HybridRetriever] Vector search failed: {e}");
            Vec::new()
        });

        let bm25_hits = bm25_results.unwrap_or_else(|e| {
            eprintln!("[HybridRetriever] BM25 search failed: {e}");
            Vec::new()
        });

        // Fuse with RRF
        let fused = rrf_fuse(&vector_hits, &bm25_hits, top_k);

        // Enrich with note metadata
        let mut results = Vec::with_capacity(fused.len());
        for item in fused {
            let meta = sqlx::query_as::<_, (String,)>("SELECT title FROM notes WHERE id = ?")
                .bind(&item.note_id)
                .fetch_optional(db)
                .await
                .map_err(SearchError::from)?;

            let note_title = meta.map_or_else(|| "Untitled".to_string(), |m| m.0);

            results.push(RetrievedChunk {
                chunk_id: item.chunk_id,
                note_id: item.note_id,
                note_title,
                text: item.text,
                heading_path: item.heading_path,
                char_start: item.char_start,
                char_end: item.char_end,
                rrf_score: item.rrf_score,
                vector_score: item.vector_score,
                bm25_score: item.bm25_score,
            });
        }

        Ok(results)
    }

    /// Vector similarity search via LanceDB.
    async fn vector_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorHit>, SearchError> {
        // Check if table exists
        let table_names = self
            .lance_conn
            .table_names()
            .execute()
            .await
            .map_err(|e| SearchError::VectorStore(e.to_string()))?;

        if !table_names.contains(&"embeddings".to_string()) {
            return Ok(Vec::new());
        }

        // Embed the query using E5 prompt structure
        let model = self.model.clone();
        let query_text = format!("query: {}", query);
        let query_vector = tokio::task::spawn_blocking(move || -> Result<Vec<f32>, SearchError> {
            let mut model = model
                .lock()
                .map_err(|e| SearchError::Embedding(format!("lock error: {e}")))?;
            let embeddings = model
                .embed(vec![query_text], None)
                .map_err(|e| SearchError::Embedding(e.to_string()))?;
            embeddings
                .into_iter()
                .next()
                .ok_or_else(|| SearchError::Embedding("no embedding produced".into()))
        })
        .await
        .map_err(|e| SearchError::Embedding(format!("embed task panicked: {e}")))??;

        // Search LanceDB
        let table = self
            .lance_conn
            .open_table("embeddings")
            .execute()
            .await
            .map_err(|e| SearchError::VectorStore(e.to_string()))?;

        let stream = table
            .query()
            .nearest_to(query_vector)
            .map_err(|e| SearchError::VectorStore(e.to_string()))?
            .limit(limit)
            .select(lancedb::query::Select::Columns(vec![
                "chunk_id".into(),
                "note_id".into(),
                "text".into(),
                "heading_path".into(),
                "char_start".into(),
                "char_end".into(),
            ]))
            .execute()
            .await
            .map_err(|e| SearchError::VectorStore(e.to_string()))?;

        let batches: Vec<RecordBatch> = stream
            .try_collect()
            .await
            .map_err(|e| SearchError::VectorStore(e.to_string()))?;

        let mut hits = Vec::new();
        for batch in &batches {
            let chunk_ids = batch
                .column_by_name("chunk_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let note_ids = batch
                .column_by_name("note_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let texts = batch
                .column_by_name("text")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let headings = batch
                .column_by_name("heading_path")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let char_starts = batch
                .column_by_name("char_start")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Int32Array>());
            let char_ends = batch
                .column_by_name("char_end")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Int32Array>());
            let distances = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>());

            let (Some(chunk_ids), Some(note_ids), Some(texts), Some(headings)) =
                (chunk_ids, note_ids, texts, headings)
            else {
                continue;
            };

            for i in 0..batch.num_rows() {
                let distance = distances.map_or(0.0, |d| d.value(i));
                let score = 1.0 / (1.0 + distance);

                hits.push(VectorHit {
                    chunk_id: chunk_ids.value(i).to_string(),
                    note_id: note_ids.value(i).to_string(),
                    text: texts.value(i).to_string(),
                    heading_path: headings.value(i).to_string(),
                    char_start: char_starts.map_or(0, |a| a.value(i)),
                    char_end: char_ends.map_or(0, |a| a.value(i)),
                    score,
                });
            }
        }

        Ok(hits)
    }
}

async fn bm25_search(
    query: &str,
    limit: usize,
    db: &SqlitePool,
) -> Result<Vec<Bm25Hit>, SearchError> {
    // FTS5 match query — escape special chars and use implicit AND
    let fts_query = sanitize_fts_query(query);

    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, (String, String, String, String, f64)>(
        r#"
        SELECT
            c.id AS chunk_id,
            c.note_id,
            c.text,
            c.heading_path,
            bm25(chunks_fts) AS score
        FROM chunks_fts
        JOIN chunks c ON c.rowid = chunks_fts.rowid
        WHERE chunks_fts MATCH ?
        ORDER BY score
        LIMIT ?
        "#,
    )
    .bind(&fts_query)
    .bind(limit as i64)
    .fetch_all(db)
    .await
    .map_err(SearchError::from)?;

    Ok(rows
        .into_iter()
        .map(|(chunk_id, note_id, text, heading_path, score)| Bm25Hit {
            chunk_id,
            note_id,
            text,
            heading_path,
            score: -score, // FTS5 bm25() returns negative; negate for ascending rank
        })
        .collect())
}

fn sanitize_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|word| {
            // Strip FTS5 special chars
            let clean: String = word
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if clean.is_empty() {
                String::new()
            } else {
                format!("\"{}\"", clean)
            }
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" OR ")
}

struct FusedResult {
    chunk_id: String,
    note_id: String,
    text: String,
    heading_path: String,
    char_start: i32,
    char_end: i32,
    rrf_score: f32,
    vector_score: f32,
    bm25_score: f32,
}

fn rrf_fuse(vector_hits: &[VectorHit], bm25_hits: &[Bm25Hit], top_k: usize) -> Vec<FusedResult> {
    // Track by chunk_id → (rrf_score, vector_score, bm25_score, hit_data)
    let mut scores: HashMap<String, FusedResult> = HashMap::new();

    // Score vector results
    for (rank, hit) in vector_hits.iter().enumerate() {
        let rrf_contribution = 1.0 / (RRF_K + rank as f32 + 1.0);
        let entry = scores.entry(hit.chunk_id.clone()).or_insert(FusedResult {
            chunk_id: hit.chunk_id.clone(),
            note_id: hit.note_id.clone(),
            text: hit.text.clone(),
            heading_path: hit.heading_path.clone(),
            char_start: hit.char_start,
            char_end: hit.char_end,
            rrf_score: 0.0,
            vector_score: 0.0,
            bm25_score: 0.0,
        });
        entry.rrf_score += rrf_contribution;
        entry.vector_score = hit.score;
    }

    // Score BM25 results
    for (rank, hit) in bm25_hits.iter().enumerate() {
        let rrf_contribution = 1.0 / (RRF_K + rank as f32 + 1.0);
        let entry = scores.entry(hit.chunk_id.clone()).or_insert(FusedResult {
            chunk_id: hit.chunk_id.clone(),
            note_id: hit.note_id.clone(),
            text: hit.text.clone(),
            heading_path: hit.heading_path.clone(),
            char_start: 0,
            char_end: 0,
            rrf_score: 0.0,
            vector_score: 0.0,
            bm25_score: 0.0,
        });
        entry.rrf_score += rrf_contribution;
        entry.bm25_score = hit.score as f32;
    }

    // Sort by RRF score descending, take top_k
    let mut results: Vec<FusedResult> = scores.into_values().collect();
    results.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(top_k);

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_fts_query() {
        assert_eq!(sanitize_fts_query("hello world"), "\"hello\" OR \"world\"");
        assert_eq!(
            sanitize_fts_query("AND OR NOT"),
            "\"AND\" OR \"OR\" OR \"NOT\""
        );
        assert_eq!(sanitize_fts_query("test-query"), "\"test-query\"");
        assert_eq!(sanitize_fts_query("   "), "");
    }

    #[test]
    fn test_rrf_fusion_empty() {
        let result = rrf_fuse(&[], &[], 10);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_rrf_fusion_vector_only() {
        let vector_hits = vec![
            VectorHit {
                chunk_id: "c1".into(),
                note_id: "n1".into(),
                text: "text1".into(),
                heading_path: "".into(),
                char_start: 0,
                char_end: 5,
                score: 0.9,
            },
            VectorHit {
                chunk_id: "c2".into(),
                note_id: "n2".into(),
                text: "text2".into(),
                heading_path: "".into(),
                char_start: 0,
                char_end: 5,
                score: 0.7,
            },
        ];
        let results = rrf_fuse(&vector_hits, &[], 10);
        assert_eq!(results.len(), 2);
        // First result should have higher RRF score (rank 0)
        assert!(results[0].rrf_score > results[1].rrf_score);
        assert_eq!(results[0].chunk_id, "c1");
    }

    #[test]
    fn test_rrf_fusion_overlap_boosts() {
        // A chunk that appears in both lists should get boosted
        let vector_hits = vec![VectorHit {
            chunk_id: "shared".into(),
            note_id: "n1".into(),
            text: "text".into(),
            heading_path: "".into(),
            char_start: 0,
            char_end: 4,
            score: 0.8,
        }];
        let bm25_hits = vec![Bm25Hit {
            chunk_id: "shared".into(),
            note_id: "n1".into(),
            text: "text".into(),
            heading_path: "".into(),
            score: 5.0,
        }];
        let bm25_only = vec![Bm25Hit {
            chunk_id: "bm25only".into(),
            note_id: "n2".into(),
            text: "other".into(),
            heading_path: "".into(),
            score: 10.0,
        }];

        let mut all_bm25 = bm25_hits;
        all_bm25.extend(bm25_only);

        let results = rrf_fuse(&vector_hits, &all_bm25, 10);
        // "shared" appears in both lists → higher RRF than "bm25only"
        assert_eq!(results[0].chunk_id, "shared");
        assert!(results[0].rrf_score > results[1].rrf_score);
    }
}
