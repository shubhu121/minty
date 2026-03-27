//! Custom error types for the RAG pipeline.
//!
//! Uses `thiserror` for ergonomic error handling with `?` propagation.
//! Each variant wraps a specific failure domain so callers can match
//! or simply display the error.

use thiserror::Error;

/// Errors that can occur during search and retrieval operations.
#[derive(Debug, Error)]
pub enum SearchError {
    /// The embedding model failed to generate vectors.
    #[error("embedding failed: {0}")]
    Embedding(String),

    /// LanceDB query or connection failure.
    #[error("vector store error: {0}")]
    VectorStore(String),

    /// SQLite metadata lookup failure.
    #[error("metadata lookup failed: {0}")]
    Metadata(#[from] sqlx::Error),

    /// The embeddings table does not exist yet (index is empty).
    #[error("index is empty — no notes have been indexed yet")]
    EmptyIndex,
}

impl From<lancedb::error::Error> for SearchError {
    fn from(e: lancedb::error::Error) -> Self {
        Self::VectorStore(e.to_string())
    }
}

impl From<anyhow::Error> for SearchError {
    fn from(e: anyhow::Error) -> Self {
        Self::Embedding(e.to_string())
    }
}

// Conversion to String for Tauri command results
impl From<SearchError> for String {
    fn from(e: SearchError) -> Self {
        e.to_string()
    }
}
