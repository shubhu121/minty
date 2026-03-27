//! Tauri commands for LLM backend management.

use serde::Serialize;
use tauri::State;

use crate::llm::backend::LlmBackend;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct OllamaStatus {
    pub available: bool,
    pub models: Vec<String>,
    pub active_model: String,
    pub completion_model: String,
}

/// Check if Ollama is running and list available models.
#[tauri::command]
pub async fn check_ollama_status(state: State<'_, AppState>) -> Result<OllamaStatus, String> {
    let available = state.ollama.is_available().await;

    let models = if available {
        state
            .ollama
            .list_models()
            .await
            .map(|m| m.into_iter().map(|info| info.name).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let active_model = state.ollama.model_name().to_string();
    let completion_model = state.ollama.completion_model_name().to_string();

    Ok(OllamaStatus {
        available,
        models,
        active_model,
        completion_model,
    })
}

/// List all models available on the Ollama server.
#[tauri::command]
pub async fn list_ollama_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state
        .ollama
        .list_models()
        .await
        .map(|m| m.into_iter().map(|info| info.name).collect())
        .map_err(|e| e.to_string())
}

/// Set the active Ollama model.
#[tauri::command]
pub async fn set_ollama_model(state: State<'_, AppState>, model: String) -> Result<(), String> {
    state.ollama.set_model(model);
    Ok(())
}

/// Set the active Ollama completion model.
#[tauri::command]
pub async fn set_ollama_completion_model(
    state: State<'_, AppState>,
    model: String,
) -> Result<(), String> {
    state.ollama.set_completion_model(model);
    Ok(())
}
