//! Tauri command wrappers for search functionality.

use serde::Deserialize;
use tauri::State;

use crate::rag::embedder::{EmbedJob, IndexingStatus};
use crate::rag::retrieval::RetrievedChunk;
use crate::rag::search::{self, Backlink};
use crate::state::AppState;

#[tauri::command]
pub async fn get_indexing_status(state: State<'_, AppState>) -> Result<IndexingStatus, String> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notes")
        .fetch_one(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    let indexed: (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT note_id) FROM chunks WHERE embedding_model != ''")
            .fetch_one(&state.db)
            .await
            .map_err(|e| e.to_string())?;

    Ok(IndexingStatus {
        total: total.0 as usize,
        indexed: indexed.0 as usize,
    })
}

#[tauri::command]
pub async fn reindex_all_notes(state: State<'_, AppState>) -> Result<usize, String> {

    sqlx::query("DELETE FROM chunks")
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;


    if let Ok(table) = state
        .hybrid_retriever
        .lance_conn
        .open_table("embeddings")
        .execute()
        .await
    {
        let _ = table.delete("note_id IS NOT NULL").await;
    }

    // 3. Queue all existing notes
    let mut count = 0;
    let notes: Vec<(String, String)> = sqlx::query_as("SELECT id, path FROM notes")
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    for (id, path) in notes {
        let full_path = state.note_engine.vault_path.join(&path);
        if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
            state.embed_worker.queue(EmbedJob {
                note_id: id,
                note_path: path,
                content,
            });
            count += 1;
        }
    }

    Ok(count)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    Hybrid,
    Semantic,
    Keyword,
}

#[tauri::command]
pub async fn semantic_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
    mode: Option<SearchMode>,
) -> Result<Vec<RetrievedChunk>, String> {
    let limit = limit.unwrap_or(10);
    let mode = mode.unwrap_or(SearchMode::Hybrid);

    match mode {
        SearchMode::Hybrid => state
            .hybrid_retriever
            .retrieve(&query, limit, &state.db)
            .await
            .map_err(|e| e.to_string()),

        SearchMode::Semantic => {
            // Use vector-only search, convert to RetrievedChunk
            let results = state
                .search_engine
                .search(&query, limit, &state.db)
                .await
                .map_err(|e| e.to_string())?;

            Ok(results
                .into_iter()
                .map(|r| RetrievedChunk {
                    chunk_id: String::new(),
                    note_id: r.note_id,
                    note_title: r.note_title,
                    text: r.chunk_text,
                    heading_path: r.heading_path,
                    char_start: r.char_start,
                    char_end: r.char_end,
                    rrf_score: r.score,
                    vector_score: r.score,
                    bm25_score: 0.0,
                })
                .collect())
        }

        SearchMode::Keyword => {
            // BM25-only via FTS5
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
            .bind(&query)
            .bind(limit as i64)
            .fetch_all(&state.db)
            .await
            .map_err(|e| e.to_string())?;

            let mut results = Vec::with_capacity(rows.len());
            for (chunk_id, note_id, text, heading_path, score) in rows {
                let meta = sqlx::query_as::<_, (String,)>("SELECT title FROM notes WHERE id = ?")
                    .bind(&note_id)
                    .fetch_optional(&state.db)
                    .await
                    .map_err(|e| e.to_string())?;

                let title = meta.map_or("Untitled".to_string(), |m| m.0);
                results.push(RetrievedChunk {
                    chunk_id,
                    note_id,
                    note_title: title,
                    text,
                    heading_path,
                    char_start: 0,
                    char_end: 0,
                    rrf_score: (-score) as f32,
                    vector_score: 0.0,
                    bm25_score: (-score) as f32,
                });
            }
            Ok(results)
        }
    }
}

#[tauri::command]
pub async fn get_related_notes(
    state: State<'_, AppState>,
    note_id: String,
    limit: Option<usize>,
) -> Result<Vec<crate::rag::related::RelatedNote>, String> {
    let limit = limit.unwrap_or(5);

    let lance_conn = &state.hybrid_retriever.lance_conn;

    crate::rag::related::get_related_notes(&note_id, limit, lance_conn, &state.db)
        .await
        .map_err(|e| e.to_string())
}

/// Get all notes that link to the given note (backlinks).
#[tauri::command]
pub async fn get_backlinks(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Vec<Backlink>, String> {
    search::get_backlinks(&note_id, &state.db)
        .await
        .map_err(|e| e.to_string())
}
