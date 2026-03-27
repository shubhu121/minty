use tauri::State;

use crate::notes::engine::{NoteContent, NoteMetadata};
use crate::state::AppState;

#[tauri::command]
pub async fn get_all_notes(state: State<'_, AppState>) -> Result<Vec<NoteMetadata>, String> {
    state
        .note_engine
        .get_all_notes()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_note(state: State<'_, AppState>, id: String) -> Result<NoteContent, String> {
    state
        .note_engine
        .get_note(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_note(
    state: State<'_, AppState>,
    title: String,
) -> Result<NoteMetadata, String> {
    state
        .note_engine
        .create_note(&title)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_note(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> Result<(), String> {
    state
        .note_engine
        .update_note(&id, &content)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_note(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .note_engine
        .delete_note(&id)
        .await
        .map_err(|e| e.to_string())
}
