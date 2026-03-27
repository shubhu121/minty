use notify::RecommendedWatcher;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::Mutex as TokioMutex;

use crate::llm::backend::ChatMessage;
use crate::llm::ollama::OllamaBackend;
use crate::notes::engine::NoteEngine;
use crate::rag::embedder::EmbedWorker;
use crate::rag::retrieval::HybridRetriever;
use crate::rag::search::SearchEngine;

/// Shared application state, managed by Tauri and accessible from all commands.
pub struct AppState {
    pub db: SqlitePool,
    pub note_engine: NoteEngine,
    /// Kept alive so the filesystem watcher doesn't drop.
    pub _watcher: Mutex<Option<RecommendedWatcher>>,
    /// Background embedding worker.
    pub embed_worker: EmbedWorker,
    /// Vector-only search engine.
    pub search_engine: SearchEngine,
    /// Hybrid vector + BM25 retrieval engine.
    pub hybrid_retriever: Arc<HybridRetriever>,
    /// Ollama LLM backend.
    pub ollama: Arc<OllamaBackend>,
    /// RAG conversation history store.
    pub conversations: TokioMutex<HashMap<String, Vec<ChatMessage>>>,
}
