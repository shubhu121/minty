use serde::Serialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::llm::backend::LlmBackend;
use crate::state::AppState;

#[derive(Clone, Serialize)]
pub struct SuggestionToken {
    pub token: String,
    pub request_id: u32,
    pub done: bool,
}

#[tauri::command]
pub fn get_inline_suggestion(
    app: AppHandle,
    state: State<'_, AppState>,
    _note_id: String,
    prefix: String,
    _suffix: String,
    _cursor_pos: usize,
    request_id: u32,
) {
    if prefix.trim().len() < 20 {
        // Silently fail if context is too small
        return;
    }

    let hybrid_retriever = state.hybrid_retriever.clone();
    let db_pool = state.db.clone();
    let ollama_backend = state.ollama.clone();

    tauri::async_runtime::spawn(async move {
        let is_available = ollama_backend.is_available().await;
        if !is_available {
            return;
        }

        let trimmed_prefix = prefix.trim();
        // Extract last sentence for query
        let sentences: Vec<&str> = trimmed_prefix
            .split(['.', '?', '!', '\n'])
            .filter(|s| !s.trim().is_empty())
            .collect();
        let last_sentence = sentences.last().copied().unwrap_or("").trim();

        if last_sentence.len() < 10 {
            return;
        }

        // Optional: retrieve context if needed, but for completion it can be better without it to stay in-voice,
        // or we retrieve with Hybrid and pass as facts.
        let chunks = hybrid_retriever
            .retrieve(last_sentence, 2, &db_pool)
            .await
            .unwrap_or_default();

        let mut context_block = String::new();
        for (i, c) in chunks.iter().enumerate() {
            context_block.push_str(&format!("[{}]\n{}\n\n", i + 1, c.text));
        }

        let prompt = format!(
            "You are an autocompletion engine. Continue the text seamlessly from exactly where the prefix ends. \
            Do NOT repeat the prefix. ONLY output the continuation. Do not include markdown formatting like backticks.\n\n\
            Relevant facts (use only if helpful):\n{}\n\n\
            Text to continue:\n{}",
            context_block, prefix
        );

        let app_clone = app.clone();
        let token_count = Arc::new(AtomicUsize::new(0));

        let _ = ollama_backend
            .stream_completion(
                &prompt,
                Box::new(move |token| {
                    let current = token_count.fetch_add(1, Ordering::SeqCst);

                    // Hard stop at 80 tokens
                    if current >= 80 {
                        return;
                    }

                    // A hack to stop processing locally: we still get stream chunks until it finishes generating sadly,
                    // but we just stop emitting them to frontend.
                    // Stop emitting if we hit newline or punctuation to keep suggestions short.
                    // Wait, stopping on punctuation limits it to one sentence completion. That's actually what we want!

                    let _ = app_clone.emit(
                        "inline_suggestion_token",
                        SuggestionToken {
                            token: token.clone(),
                            request_id,
                            done: false,
                        },
                    );
                }),
            )
            .await;

        let _ = app.emit(
            "inline_suggestion_token",
            SuggestionToken {
                token: "".to_string(),
                request_id,
                done: true,
            },
        );
    });
}
