//! Semantic search engine for the RAG pipeline.
//!
//! Embeds a query string using fastembed, searches LanceDB for the nearest
//! chunk vectors, then enriches results with note metadata from SQLite.

use arrow_array::{Array, RecordBatch, StringArray};
use fastembed::EmbeddingModel;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::PathBuf;

use super::embedding_model::{load_shared_text_embedding, SharedTextEmbedding};
use super::errors::SearchError;

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    /// The note this chunk belongs to.
    pub note_id: String,
    /// The title of the note.
    pub note_title: String,
    /// Relative path of the note in the vault.
    pub note_path: String,
    /// The matched chunk text.
    pub chunk_text: String,
    /// Heading breadcrumb for context (e.g. "Introduction > Background").
    pub heading_path: String,
    /// Character offset into the note (for linking to position).
    pub char_start: i32,
    pub char_end: i32,
    /// Similarity score (lower _distance_ = better match).
    pub score: f32,
}

/// A backlink: a note that links to the given note.
#[derive(Debug, Clone, Serialize)]
pub struct Backlink {
    pub source_id: String,
    pub source_title: String,
    pub source_path: String,
    pub anchor_text: String,
    pub link_type: String,
}

/// The search engine holds a LanceDB connection and a reference to the
/// fastembed model (shared with the embedder via `Arc<Mutex>`).
pub struct SearchEngine {
    lance_conn: lancedb::Connection,
    model: SharedTextEmbedding,
}

impl SearchEngine {
    /// Create a new search engine.
    ///
    /// Initialises a separate fastembed model instance for query embedding.
    /// The model is loaded in a blocking context to avoid stalling the
    /// async runtime.
    pub async fn new(
        lance_db_path: PathBuf,
        model_cache_path: PathBuf,
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

    /// Perform semantic search: embed the query, find nearest vectors,
    /// then enrich results with note metadata from SQLite.
    ///
    /// # Arguments
    /// * `query` — natural-language search query
    /// * `limit` — maximum number of results to return
    /// * `db` — SQLite pool for metadata enrichment
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        db: &SqlitePool,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Check if the embeddings table exists
        let table_names = self
            .lance_conn
            .table_names()
            .execute()
            .await
            .map_err(|e| SearchError::VectorStore(e.to_string()))?;

        if !table_names.contains(&"embeddings".to_string()) {
            return Err(SearchError::EmptyIndex);
        }

        // 1. Embed the query in spawn_blocking
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

        // 2. Search LanceDB
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

        // 3. Parse results from Arrow batches
        let mut raw_results: Vec<RawSearchHit> = Vec::new();
        for batch in &batches {
            let note_ids = batch
                .column_by_name("note_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| SearchError::VectorStore("missing note_id column".into()))?;

            let texts = batch
                .column_by_name("text")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| SearchError::VectorStore("missing text column".into()))?;

            let headings = batch
                .column_by_name("heading_path")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| SearchError::VectorStore("missing heading_path column".into()))?;

            let char_starts = batch
                .column_by_name("char_start")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Int32Array>())
                .ok_or_else(|| SearchError::VectorStore("missing char_start column".into()))?;

            let char_ends = batch
                .column_by_name("char_end")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Int32Array>())
                .ok_or_else(|| SearchError::VectorStore("missing char_end column".into()))?;

            // _distance is added by LanceDB for nearest_to queries
            let distances = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>());

            for i in 0..batch.num_rows() {
                let distance = distances.map_or(0.0, |d| d.value(i));
                // Convert distance to a 0..1 similarity score
                // L2 distance: similarity = 1 / (1 + distance)
                let score = 1.0 / (1.0 + distance);

                raw_results.push(RawSearchHit {
                    note_id: note_ids.value(i).to_string(),
                    chunk_text: texts.value(i).to_string(),
                    heading_path: headings.value(i).to_string(),
                    char_start: char_starts.value(i),
                    char_end: char_ends.value(i),
                    score,
                });
            }
        }

        // 4. Enrich with note metadata from SQLite
        let mut results = Vec::with_capacity(raw_results.len());
        for hit in raw_results {
            let meta =
                sqlx::query_as::<_, (String, String)>("SELECT title, path FROM notes WHERE id = ?")
                    .bind(&hit.note_id)
                    .fetch_optional(db)
                    .await?;

            if let Some((title, path)) = meta {
                results.push(SearchResult {
                    note_id: hit.note_id,
                    note_title: title,
                    note_path: path,
                    chunk_text: hit.chunk_text,
                    heading_path: hit.heading_path,
                    char_start: hit.char_start,
                    char_end: hit.char_end,
                    score: hit.score,
                });
            }
        }

        Ok(results)
    }
}

/// Get all backlinks for a note (notes that link _to_ this note).
pub async fn get_backlinks(note_id: &str, db: &SqlitePool) -> Result<Vec<Backlink>, SearchError> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        r#"
        SELECT n.id, n.title, n.path, l.anchor_text, l.link_type
        FROM links l
        JOIN notes n ON n.id = l.source_id
        WHERE l.target_id = ?
        ORDER BY n.title
        "#,
    )
    .bind(note_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, title, path, anchor, link_type)| Backlink {
            source_id: id,
            source_title: title,
            source_path: path,
            anchor_text: anchor,
            link_type,
        })
        .collect())
}

// Internal intermediate type for raw LanceDB hits before enrichment
struct RawSearchHit {
    note_id: String,
    chunk_text: String,
    heading_path: String,
    char_start: i32,
    char_end: i32,
    score: f32,
}
