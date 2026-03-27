//! RAG generation engine: orchestrates retrieval → augmentation → streaming
//! LLM generation with inline citation detection.

use std::sync::Arc;

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

use crate::llm::backend::{ChatMessage, LlmBackend, LlmError};

use super::augmentation::{self, AssembledContext, SourceInfo};
use super::retrieval::HybridRetriever;

// ─── Event Types ─────────────────────────────────────────────

/// Event emitted before generation starts, listing source notes.
#[derive(Debug, Clone, Serialize)]
pub struct RagSourcesEvent {
    pub conversation_id: String,
    pub sources: Vec<SourceInfo>,
}

/// Individual streaming event during generation.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum StreamEvent {
    /// A text token from the LLM.
    #[serde(rename = "token")]
    Token(String),
    /// A resolved citation reference.
    #[serde(rename = "citation")]
    Citation {
        label: String,
        note_id: String,
        note_title: String,
    },
    /// Generation complete.
    #[serde(rename = "done")]
    Done,
    /// An error occurred.
    #[serde(rename = "error")]
    Error(String),
}

/// Wrapper that includes conversation_id with each stream event.
#[derive(Debug, Clone, Serialize)]
pub struct RagStreamEvent {
    pub conversation_id: String,
    pub event: StreamEvent,
}

pub struct RagEngine {
    retriever: Arc<HybridRetriever>,
    backend: Arc<dyn LlmBackend>,
}

impl RagEngine {
    pub fn new(retriever: Arc<HybridRetriever>, backend: Arc<dyn LlmBackend>) -> Self {
        Self { retriever, backend }
    }

    pub async fn ask(
        &self,
        app: &AppHandle,
        question: &str,
        conversation_id: &str,
        history: &[ChatMessage],
        db: &SqlitePool,
    ) -> Result<String, LlmError> {
        // 1. Retrieve relevant chunks
        let chunks = self
            .retriever
            .retrieve(question, 8, db)
            .await
            .map_err(|e| LlmError::Request(e.to_string()))?;

        if chunks.is_empty() {
            let msg = "I couldn't find any relevant information in your notes. \
                       Try rephrasing your question or make sure you have notes on this topic."
                .to_string();

            let _ = app.emit(
                "rag_stream",
                RagStreamEvent {
                    conversation_id: conversation_id.to_string(),
                    event: StreamEvent::Token(msg.clone()),
                },
            );
            let _ = app.emit(
                "rag_stream",
                RagStreamEvent {
                    conversation_id: conversation_id.to_string(),
                    event: StreamEvent::Done,
                },
            );
            return Ok(msg);
        }

        // 2. Assemble context
        let context = augmentation::assemble_context(chunks, None);
        if context.context_block.trim().is_empty() {
            let msg = "I found matching notes, but I couldn't assemble usable context from them."
                .to_string();

            let _ = app.emit(
                "rag_stream",
                RagStreamEvent {
                    conversation_id: conversation_id.to_string(),
                    event: StreamEvent::Token(msg.clone()),
                },
            );
            let _ = app.emit(
                "rag_stream",
                RagStreamEvent {
                    conversation_id: conversation_id.to_string(),
                    event: StreamEvent::Done,
                },
            );
            return Ok(msg);
        }

        // 3. Emit sources to frontend
        let _ = app.emit(
            "rag_sources",
            RagSourcesEvent {
                conversation_id: conversation_id.to_string(),
                sources: context.sources.clone(),
            },
        );

        // 4. Build messages array
        let mut messages = vec![ChatMessage::system(&context.system_prompt)];

        // Include conversation history (last 6 messages)
        let history_window = if history.len() > 6 {
            &history[history.len() - 6..]
        } else {
            history
        };
        messages.extend_from_slice(history_window);

        // Add the current question
        messages.push(ChatMessage::user(question));

        // 5. Stream generation with citation detection
        let full_response =
            stream_with_citations(&*self.backend, messages, &context, app, conversation_id).await?;

        Ok(full_response)
    }
}

async fn stream_with_citations(
    backend: &dyn LlmBackend,
    messages: Vec<ChatMessage>,
    context: &AssembledContext,
    app: &AppHandle,
    conversation_id: &str,
) -> Result<String, LlmError> {
    let conv_id = conversation_id.to_string();
    let source_map = context.source_map.clone();
    let app_clone = app.clone();

    // Shared state for the citation state machine
    let full_response = Arc::new(std::sync::Mutex::new(String::new()));
    let response_clone = full_response.clone();

    // Citation buffer state
    let bracket_buffer = Arc::new(std::sync::Mutex::new(String::new()));
    let in_bracket = Arc::new(std::sync::Mutex::new(false));

    let bb = bracket_buffer.clone();
    let ib = in_bracket.clone();
    let sm = source_map;

    let on_token: Box<dyn Fn(String) + Send> = Box::new(move |token: String| {
        let mut full = response_clone.lock().unwrap();
        full.push_str(&token);

        let mut buffer = bb.lock().unwrap();
        let mut inside = ib.lock().unwrap();

        for ch in token.chars() {
            if ch == '[' && !*inside {
                *inside = true;
                buffer.clear();
                buffer.push(ch);
            } else if *inside {
                buffer.push(ch);
                if ch == ']' {
                    // Check if buffer matches a source label
                    let inner = buffer[1..buffer.len() - 1].trim();
                    if let Some(source) = sm.get(inner) {
                        // Emit citation event
                        let _ = app_clone.emit(
                            "rag_stream",
                            RagStreamEvent {
                                conversation_id: conv_id.clone(),
                                event: StreamEvent::Citation {
                                    label: source.label.clone(),
                                    note_id: source.note_id.clone(),
                                    note_title: source.note_title.clone(),
                                },
                            },
                        );
                    } else {
                        // Not a valid citation, emit as plain text
                        let _ = app_clone.emit(
                            "rag_stream",
                            RagStreamEvent {
                                conversation_id: conv_id.clone(),
                                event: StreamEvent::Token(buffer.clone()),
                            },
                        );
                    }
                    buffer.clear();
                    *inside = false;
                } else if buffer.len() > 20 {
                    // Too long for a citation label, flush as text
                    let _ = app_clone.emit(
                        "rag_stream",
                        RagStreamEvent {
                            conversation_id: conv_id.clone(),
                            event: StreamEvent::Token(buffer.clone()),
                        },
                    );
                    buffer.clear();
                    *inside = false;
                }
            } else {
                // Normal character outside brackets
                let _ = app_clone.emit(
                    "rag_stream",
                    RagStreamEvent {
                        conversation_id: conv_id.clone(),
                        event: StreamEvent::Token(ch.to_string()),
                    },
                );
            }
        }
    });

    backend.stream_generate(messages, on_token).await?;

    // Flush any remaining buffer
    {
        let buffer = bracket_buffer.lock().unwrap();
        if !buffer.is_empty() {
            let _ = app.emit(
                "rag_stream",
                RagStreamEvent {
                    conversation_id: conversation_id.to_string(),
                    event: StreamEvent::Token(buffer.clone()),
                },
            );
        }
    }

    // Emit done
    let _ = app.emit(
        "rag_stream",
        RagStreamEvent {
            conversation_id: conversation_id.to_string(),
            event: StreamEvent::Done,
        },
    );

    let result = full_response.lock().unwrap().clone();
    Ok(result)
}
