use anyhow::{Context, Result};
use arrow_array::{Float32Array, RecordBatch, RecordBatchIterator, RecordBatchReader, StringArray};
use arrow_schema::{DataType, Field, Schema};
use fastembed::EmbeddingModel;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::chunker;
use super::embedding_model::{load_shared_text_embedding, SharedTextEmbedding};
use crate::notes::parser;

/// Dimensions for MultilingualE5Small model.
const EMBEDDING_DIM: usize = 384;
const MODEL_NAME: &str = "intfloat/multilingual-e5-small";

/// A job sent to the embedding worker.
#[derive(Debug)]
pub struct EmbedJob {
    pub note_id: String,
    pub note_path: String,
    pub content: String,
}

/// Indexing status for the frontend progress indicator.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexingStatus {
    pub total: usize,
    pub indexed: usize,
}

/// The embedding worker that processes notes in the background.
pub struct EmbedWorker {
    pub sender: mpsc::Sender<EmbedJob>,
}

impl EmbedWorker {
    /// Start the embedding worker. Returns the worker (with sender) and spawns
    /// the background processing loop.
    pub async fn start(
        db: SqlitePool,
        lance_db_path: PathBuf,
        model_cache_path: PathBuf,
    ) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<EmbedJob>(256);

        // Initialize LanceDB connection
        let lance_path_str = lance_db_path.to_string_lossy().to_string();
        let lance_conn = lancedb::connect(&lance_path_str)
            .execute()
            .await
            .context("Failed to connect to LanceDB")?;

        // Ensure the embeddings table exists
        ensure_lance_table(&lance_conn).await?;

        // Load the model before startup continues so later components do not
        // race against a half-written cache on first launch.
        let model =
            load_shared_text_embedding(EmbeddingModel::MultilingualE5Small, Some(model_cache_path))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to initialize embedding model: {e}"))?;
        println!("[EmbedWorker] Embedding model loaded successfully");

        // Spawn the worker loop
        tauri::async_runtime::spawn(embed_worker_loop(rx, db, lance_conn, model));

        Ok(Self { sender: tx })
    }

    /// Queue a note for embedding.
    pub fn queue(&self, job: EmbedJob) {
        let sender = self.sender.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = sender.send(job).await {
                eprintln!("[EmbedWorker] Failed to queue job: {}", e);
            }
        });
    }
}

/// Ensure the LanceDB table exists with the correct schema.
async fn ensure_lance_table(conn: &lancedb::Connection) -> Result<()> {
    let table_names = conn.table_names().execute().await?;

    if !table_names.contains(&"embeddings".to_string()) {
        // Create table with an empty initial batch
        let schema = lance_schema();

        let chunk_id: Vec<String> = vec![];
        let note_id: Vec<String> = vec![];
        let text: Vec<String> = vec![];
        let heading_path: Vec<String> = vec![];
        let char_start: Vec<i32> = vec![];
        let char_end: Vec<i32> = vec![];
        let vector: Vec<f32> = vec![];

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(chunk_id)),
                Arc::new(StringArray::from(note_id)),
                Arc::new(StringArray::from(text)),
                Arc::new(StringArray::from(heading_path)),
                Arc::new(arrow_array::Int32Array::from(char_start)),
                Arc::new(arrow_array::Int32Array::from(char_end)),
                Arc::new(make_fixed_size_list_array(
                    Float32Array::from(vector),
                    EMBEDDING_DIM as i32,
                )),
            ],
        )?;

        let batches = RecordBatchIterator::new(vec![Ok(batch)], schema);
        let reader: Box<dyn RecordBatchReader + Send> = Box::new(batches);
        conn.create_table("embeddings", reader)
            .execute()
            .await
            .context("Failed to create LanceDB embeddings table")?;

        println!("[EmbedWorker] Created LanceDB embeddings table");
    }

    Ok(())
}

/// The Arrow schema for the LanceDB embeddings table.
fn lance_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("chunk_id", DataType::Utf8, false),
        Field::new("note_id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("heading_path", DataType::Utf8, false),
        Field::new("char_start", DataType::Int32, false),
        Field::new("char_end", DataType::Int32, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIM as i32,
            ),
            false,
        ),
    ]))
}

/// Build a FixedSizeListArray from flat float values.
fn make_fixed_size_list_array(values: Float32Array, size: i32) -> arrow_array::FixedSizeListArray {
    let field = Arc::new(Field::new("item", DataType::Float32, true));
    arrow_array::FixedSizeListArray::new(field, size, Arc::new(values), None)
}

/// Background loop that processes embed jobs.
async fn embed_worker_loop(
    mut rx: mpsc::Receiver<EmbedJob>,
    db: SqlitePool,
    lance_conn: lancedb::Connection,
    model: SharedTextEmbedding,
) {
    println!("[EmbedWorker] Worker started, waiting for jobs...");

    loop {
        // Wait for at least one job
        let job = match rx.recv().await {
            Some(job) => job,
            None => {
                println!("[EmbedWorker] Channel closed, worker stopping");
                break;
            }
        };

        // Drain additional jobs (batch up to 32)
        let mut jobs = vec![job];
        while jobs.len() < 32 {
            match rx.try_recv() {
                Ok(job) => jobs.push(job),
                Err(_) => break,
            }
        }

        println!("[EmbedWorker] Processing batch of {} jobs", jobs.len());

        for job in jobs {
            if let Err(e) = process_job(&job, &db, &lance_conn, &model).await {
                eprintln!("[EmbedWorker] Error processing note {}: {}", job.note_id, e);
            }
        }
    }
}

/// Process a single embed job: parse → chunk → embed → store.
async fn process_job(
    job: &EmbedJob,
    db: &SqlitePool,
    lance_conn: &lancedb::Connection,
    model: &SharedTextEmbedding,
) -> Result<()> {
    let _ = &job.note_path;
    let parsed = parser::parse_note(&job.content);
    let chunks = chunker::chunk_document(&parsed.body, &parsed.headings);

    if chunks.is_empty() {
        return Ok(());
    }

    // Delete old chunks for this note from SQLite
    sqlx::query("DELETE FROM chunks WHERE note_id = ?")
        .bind(&job.note_id)
        .execute(db)
        .await?;

    // Delete old vectors from LanceDB
    let lance_table = lance_conn.open_table("embeddings").execute().await?;
    let delete_filter = format!("note_id = '{}'", job.note_id.replace('\'', "''"));
    let _ = lance_table.delete(&delete_filter).await; // Ignore error if no rows

    // Prepare texts for embedding with 'passage: ' prefix for E5
    let texts: Vec<String> = chunks
        .iter()
        .map(|c| format!("passage: {}", c.text))
        .collect();
    let chunk_ids: Vec<String> = chunks
        .iter()
        .map(|_| uuid::Uuid::new_v4().to_string())
        .collect();

    // Run embedding in spawn_blocking (CPU-heavy ONNX inference)
    let model_clone = model.clone();
    let texts_clone = texts.clone();
    let embeddings = tokio::task::spawn_blocking(move || -> Result<Vec<Vec<f32>>> {
        let mut model = model_clone
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let embeddings = model
            .embed(texts_clone, Some(32))
            .context("Embedding generation failed")?;
        Ok(embeddings)
    })
    .await??;

    // Insert chunks into SQLite
    let now = chrono::Utc::now().timestamp();
    for (i, chunk) in chunks.iter().enumerate() {
        sqlx::query(
            "INSERT INTO chunks (id, note_id, chunk_index, text, char_start, char_end, heading_path, embedding_model, indexed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&chunk_ids[i])
        .bind(&job.note_id)
        .bind(chunk.chunk_index as i64)
        .bind(&chunk.text)
        .bind(chunk.char_start as i64)
        .bind(chunk.char_end as i64)
        .bind(&chunk.heading_path)
        .bind(MODEL_NAME)
        .bind(now)
        .execute(db)
        .await?;
    }

    // Insert vectors into LanceDB
    let schema = lance_schema();

    let all_chunk_ids: Vec<&str> = chunk_ids.iter().map(|s| s.as_str()).collect();
    let all_note_ids: Vec<&str> = chunks.iter().map(|_| job.note_id.as_str()).collect();
    let all_texts: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    let all_heading_paths: Vec<&str> = chunks.iter().map(|c| c.heading_path.as_str()).collect();
    let all_char_starts: Vec<i32> = chunks.iter().map(|c| c.char_start as i32).collect();
    let all_char_ends: Vec<i32> = chunks.iter().map(|c| c.char_end as i32).collect();

    // Flatten all embeddings into a single Vec<f32>
    let flat_vectors: Vec<f32> = embeddings.into_iter().flatten().collect();

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(all_chunk_ids)),
            Arc::new(StringArray::from(all_note_ids)),
            Arc::new(StringArray::from(all_texts)),
            Arc::new(StringArray::from(all_heading_paths)),
            Arc::new(arrow_array::Int32Array::from(all_char_starts)),
            Arc::new(arrow_array::Int32Array::from(all_char_ends)),
            Arc::new(make_fixed_size_list_array(
                Float32Array::from(flat_vectors),
                EMBEDDING_DIM as i32,
            )),
        ],
    )?;

    let batches = RecordBatchIterator::new(vec![Ok(batch)], schema);
    let reader: Box<dyn RecordBatchReader + Send> = Box::new(batches);
    let lance_table = lance_conn.open_table("embeddings").execute().await?;
    lance_table
        .add(reader)
        .execute()
        .await
        .context("Failed to add vectors to LanceDB")?;

    println!(
        "[EmbedWorker] Indexed note '{}' — {} chunks",
        job.note_id,
        chunks.len()
    );

    Ok(())
}
