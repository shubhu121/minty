//! Getting related notes visually via vector average similarity.

use arrow_array::{Array, Float32Array, RecordBatch};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use serde::Serialize;
use sqlx::SqlitePool;

use super::errors::SearchError;

/// A related note response payload.
#[derive(Debug, Clone, Serialize)]
pub struct RelatedNote {
    pub note_id: String,
    pub title: String,
    pub preview: String,
    pub similarity_score: f32, // 0.0 to 1.0
}

pub async fn get_related_notes(
    note_id: &str,
    limit: usize,
    lance_conn: &lancedb::Connection,
    db: &SqlitePool,
) -> Result<Vec<RelatedNote>, SearchError> {
    // 1. Check if embeddings table exists
    let table_names = lance_conn
        .table_names()
        .execute()
        .await
        .map_err(|e| SearchError::VectorStore(e.to_string()))?;

    if !table_names.contains(&"embeddings".to_string()) {
        return Ok(Vec::new());
    }

    let table = lance_conn
        .open_table("embeddings")
        .execute()
        .await
        .map_err(|e| SearchError::VectorStore(e.to_string()))?;

    // 2. Fetch all chunk vectors for the target note
    let stream = table
        .query()
        .select(lancedb::query::Select::Columns(vec![
            "note_id".into(),
            "vector".into(),
        ]))
        .execute()
        .await
        .map_err(|e| SearchError::VectorStore(e.to_string()))?;

    let batches: Vec<RecordBatch> = stream
        .try_collect()
        .await
        .map_err(|e| SearchError::VectorStore(e.to_string()))?;

    let mut all_vectors: Vec<Vec<f32>> = Vec::new();

    for batch in batches {
        let note_ids_col = batch
            .column_by_name("note_id")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>());
        let vectors_col = batch
            .column_by_name("vector")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::FixedSizeListArray>());

        if let (Some(note_ids), Some(list_arr)) = (note_ids_col, vectors_col) {
            let values = list_arr.values();
            if let Some(float_arr) = values.as_any().downcast_ref::<Float32Array>() {
                for i in 0..list_arr.len() {
                    // Manual filter for target note
                    if note_ids.value(i) != note_id {
                        continue;
                    }

                    let start = list_arr.value_offset(i) as usize;
                    let len = list_arr.value_length() as usize;
                    let mut vec = Vec::with_capacity(len);
                    for j in 0..len {
                        vec.push(float_arr.value(start + j));
                    }
                    all_vectors.push(vec);
                }
            }
        }
    }

    if all_vectors.is_empty() {
        return Ok(Vec::new());
    }

    // 3. Compute the average vector
    let dim = all_vectors[0].len();
    let mut avg_vector = vec![0.0f32; dim];
    for vec in &all_vectors {
        for (i, val) in vec.iter().enumerate() {
            avg_vector[i] += val;
        }
    }
    let count = all_vectors.len() as f32;
    for val in &mut avg_vector {
        *val /= count;
    }

    // L2 Normalize
    let magnitude: f32 = avg_vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        for val in &mut avg_vector {
            *val /= magnitude;
        }
    }

    let search_stream = table
        .query()
        .nearest_to(avg_vector)
        .map_err(|e| SearchError::VectorStore(e.to_string()))?
        .limit(limit * 5)
        .select(lancedb::query::Select::Columns(vec![
            "note_id".into(),
            "text".into(),
        ]))
        .execute()
        .await
        .map_err(|e| SearchError::VectorStore(e.to_string()))?;

    let search_batches: Vec<RecordBatch> = search_stream
        .try_collect()
        .await
        .map_err(|e| SearchError::VectorStore(e.to_string()))?;

    let mut unique_notes: Vec<RelatedNote> = Vec::new();
    let mut seen_notes: std::collections::HashSet<String> = std::collections::HashSet::new();

    for batch in search_batches {
        let note_ids = batch
            .column_by_name("note_id")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>());
        let texts = batch
            .column_by_name("text")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>());
        let distances = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>());

        let (Some(note_ids), Some(texts), Some(distances)) = (note_ids, texts, distances) else {
            continue;
        };

        for i in 0..batch.num_rows() {
            let n_id = note_ids.value(i).to_string();

            // Exclude self and already seen notes
            if n_id == note_id || seen_notes.contains(&n_id) {
                continue;
            }

            let dist = distances.value(i);
            let score = 1.0 / (1.0 + dist); // Convert L2 distance to 0-1 similarity

            let meta = sqlx::query_as::<_, (String,)>("SELECT title FROM notes WHERE id = ?")
                .bind(&n_id)
                .fetch_optional(db)
                .await
                .map_err(SearchError::from)?;

            let title = meta.map_or("Untitled".to_string(), |m| m.0);

            // Preview text (first 80 chars)
            let full_text = texts.value(i).to_string();
            let preview = if full_text.len() > 80 {
                format!("{}...", &full_text[0..77])
            } else {
                full_text
            };

            unique_notes.push(RelatedNote {
                note_id: n_id.clone(),
                title,
                preview,
                similarity_score: score,
            });
            seen_notes.insert(n_id);

            if unique_notes.len() >= limit {
                break;
            }
        }

        if unique_notes.len() >= limit {
            break;
        }
    }

    unique_notes.sort_by(|a, b| {
        b.similarity_score
            .partial_cmp(&a.similarity_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(unique_notes)
}
