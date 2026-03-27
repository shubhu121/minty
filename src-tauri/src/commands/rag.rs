//! Tauri commands for the RAG chat pipeline.

use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::llm::backend::{ChatMessage, LlmBackend};
use crate::rag::generation::{RagEngine, RagStreamEvent, StreamEvent};
use crate::state::AppState;

#[tauri::command]
pub async fn ask_notes(
    app: AppHandle,
    state: State<'_, AppState>,
    question: String,
    conversation_id: String,
) -> Result<(), String> {
    // Get conversation history
    let history = {
        let store = state.conversations.lock().await;
        store.get(&conversation_id).cloned().unwrap_or_default()
    };

    let rag_engine = RagEngine::new(
        state.hybrid_retriever.clone(),
        state.ollama.clone() as Arc<dyn LlmBackend>,
    );

    // Run the pipeline
    let answer = rag_engine
        .ask(&app, &question, &conversation_id, &history, &state.db)
        .await
        .map_err(|e| {
            let message = e.to_string();
            let _ = app.emit(
                "rag_stream",
                RagStreamEvent {
                    conversation_id: conversation_id.clone(),
                    event: StreamEvent::Error(message.clone()),
                },
            );
            let _ = app.emit(
                "rag_stream",
                RagStreamEvent {
                    conversation_id: conversation_id.clone(),
                    event: StreamEvent::Done,
                },
            );
            message
        })?;

    // Store messages in conversation history
    {
        let mut store = state.conversations.lock().await;
        let conv = store.entry(conversation_id).or_default();
        conv.push(ChatMessage::user(&question));
        conv.push(ChatMessage::assistant(&answer));

        // Keep max 20 messages per conversation
        if conv.len() > 20 {
            let excess = conv.len() - 20;
            conv.drain(..excess);
        }
    }

    Ok(())
}
