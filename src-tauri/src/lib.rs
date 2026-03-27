// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

mod commands;
mod db;
mod llm;
mod notes;
mod rag;
mod state;

use crate::notes::engine::NoteEngine;
use state::AppState;
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub struct WritingFile {
    pub name: String,
    pub text: String,
    pub font: String,
    pub font_size: u32,
    pub theme: String,
}

#[tauri::command]
fn get_user_folder(app_handle: AppHandle) -> Result<PathBuf, String> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    let user_folder = app_dir.join("user_data");
    fs::create_dir_all(&user_folder).map_err(|e| e.to_string())?;
    Ok(user_folder)
}

fn legacy_editor_mirror_root(app_dir: &Path) -> PathBuf {
    app_dir.join("vault").join("writesimply")
}

fn editor_mirror_root(app_dir: &Path) -> PathBuf {
    app_dir.join("vault").join("minty")
}

fn migrate_legacy_editor_mirror(app_dir: &Path) -> Result<(), String> {
    let legacy_root = legacy_editor_mirror_root(app_dir);
    let current_root = editor_mirror_root(app_dir);

    if legacy_root.exists() && !current_root.exists() {
        if let Some(parent) = current_root.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::rename(&legacy_root, &current_root).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn mirrored_note_path(app_dir: &Path, name: &str) -> PathBuf {
    editor_mirror_root(app_dir).join(format!("{}.md", name))
}

fn mirror_editor_file_to_vault(app_dir: &Path, name: &str, text: &str) -> Result<(), String> {
    let note_path = mirrored_note_path(app_dir, name);
    if let Some(parent) = note_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(note_path, text).map_err(|e| e.to_string())
}

fn remove_mirrored_editor_item(app_dir: &Path, name: &str) -> Result<(), String> {
    for root in [editor_mirror_root(app_dir), legacy_editor_mirror_root(app_dir)] {
        let note_path = root.join(format!("{}.md", name));
        if note_path.exists() {
            fs::remove_file(&note_path).map_err(|e| e.to_string())?;
        }

        let dir_path = root.join(name);
        if dir_path.exists() && dir_path.is_dir() {
            fs::remove_dir_all(dir_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn collect_json_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, out)?;
        } else if path.extension().map(|ext| ext == "json").unwrap_or(false) {
            out.push(path);
        }
    }

    Ok(())
}

fn sync_editor_files_to_vault(app_dir: &Path) -> Result<usize, String> {
    let user_folder = app_dir.join("user_data");
    fs::create_dir_all(&user_folder).map_err(|e| e.to_string())?;

    let mut json_files = Vec::new();
    collect_json_files(&user_folder, &mut json_files)?;

    let mut mirrored = 0;
    for json_path in json_files {
        let relative = match json_path.strip_prefix(&user_folder) {
            Ok(path) => path.with_extension(""),
            Err(err) => {
                eprintln!(
                    "[Smart Notes] Failed to resolve editor file path {}: {}",
                    json_path.display(),
                    err
                );
                continue;
            }
        };

        let name = relative.to_string_lossy().replace("\\", "/");
        let contents = match fs::read_to_string(&json_path) {
            Ok(contents) => contents,
            Err(err) => {
                eprintln!(
                    "[Smart Notes] Failed to read editor file {}: {}",
                    json_path.display(),
                    err
                );
                continue;
            }
        };

        let writing_file: WritingFile = match serde_json::from_str(&contents) {
            Ok(file) => file,
            Err(err) => {
                eprintln!(
                    "[Smart Notes] Failed to parse editor file {}: {}",
                    json_path.display(),
                    err
                );
                continue;
            }
        };

        if let Err(err) = mirror_editor_file_to_vault(app_dir, &name, &writing_file.text) {
            eprintln!(
                "[Smart Notes] Failed to mirror editor file {}: {}",
                json_path.display(),
                err
            );
            continue;
        }

        mirrored += 1;
    }

    Ok(mirrored)
}

#[tauri::command]
fn save_file(app_handle: AppHandle, file: WritingFile) -> Result<String, String> {
    let app_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let user_folder = get_user_folder(app_handle)?;
    let file_path = user_folder.join(format!("{}.json", file.name));

    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let serialized = serde_json::to_string(&file).map_err(|e| e.to_string())?;
    fs::write(&file_path, serialized).map_err(|e| e.to_string())?;
    mirror_editor_file_to_vault(&app_dir, &file.name, &file.text)?;

    Ok(format!("File '{}' saved successfully!", file.name))
}

#[tauri::command]
fn load_file(app_handle: AppHandle, name: String) -> Result<WritingFile, String> {
    let user_folder = get_user_folder(app_handle)?;
    let file_path = user_folder.join(format!("{}.json", name));

    if !file_path.exists() {
        return Err("File not found".into());
    }

    let mut file = File::open(file_path).map_err(|e| e.to_string())?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| e.to_string())?;

    let writing_file: WritingFile = serde_json::from_str(&contents).map_err(|e| e.to_string())?;
    Ok(writing_file)
}

fn list_files_recursive(dir: &PathBuf, base_dir: &PathBuf) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                if let Ok(relative) = path.strip_prefix(base_dir) {
                    let path_str = relative.to_string_lossy().replace("\\", "/");
                    files.push(format!("{}/", path_str));
                }
                files.extend(list_files_recursive(&path, base_dir)?);
            } else {
                if let Some(extension) = path.extension() {
                    if extension == "json" {
                        if let Ok(relative) = path.strip_prefix(base_dir) {
                            if let Some(stem) = relative.file_stem() {
                                // need to reconstruct the path with the name but without extension for the command interface
                                // actually, let's return the full relative path without extension
                                // e.g. "folder/note"
                                let parent = relative
                                    .parent()
                                    .unwrap_or(std::path::Path::new(""))
                                    .to_string_lossy();
                                let name = stem.to_string_lossy();
                                if parent.is_empty() {
                                    files.push(name.to_string());
                                } else {
                                    files.push(format!("{}/{}", parent, name));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(files)
}

#[tauri::command]
fn list_files(app_handle: AppHandle) -> Result<Vec<String>, String> {
    let user_folder = get_user_folder(app_handle)?;
    list_files_recursive(&user_folder, &user_folder)
}

#[tauri::command]
fn create_folder(app_handle: AppHandle, name: String) -> Result<String, String> {
    let user_folder = get_user_folder(app_handle)?;
    let folder_path = user_folder.join(&name);

    if folder_path.exists() {
        return Err("Folder already exists".into());
    }

    fs::create_dir_all(&folder_path).map_err(|e| e.to_string())?;
    Ok(format!("Folder '{}' created successfully!", name))
}

#[tauri::command]
fn delete_item(app_handle: AppHandle, name: String) -> Result<String, String> {
    let app_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let user_folder = get_user_folder(app_handle)?;
    // Check if it's a file (with .json)
    let file_path = user_folder.join(format!("{}.json", name));

    if file_path.exists() {
        fs::remove_file(file_path).map_err(|e| e.to_string())?;
        remove_mirrored_editor_item(&app_dir, &name)?;
        return Ok(format!("File '{}' deleted successfully!", name));
    }

    // Check if it's a directory (raw name)
    let dir_path = user_folder.join(&name);
    if dir_path.exists() && dir_path.is_dir() {
        fs::remove_dir_all(dir_path).map_err(|e| e.to_string())?;
        remove_mirrored_editor_item(&app_dir, &name)?;
        return Ok(format!("Folder '{}' deleted successfully!", name));
    }

    Err("Item not found".into())
}

#[derive(Default)]
struct AudioState {
    current_process: Option<std::process::Child>,
    current_song_path: Option<String>,
}

#[tauri::command]
fn play_audio(path: String, state: tauri::State<Mutex<AudioState>>) -> Result<(), String> {
    let mut audio_state = state.lock().map_err(|e| e.to_string())?;

    // Stop any currently playing audio
    if let Some(mut process) = audio_state.current_process.take() {
        let _ = process.kill();
    }

    // Play the new audio file using the system's native player
    #[cfg(target_os = "linux")]
    let result = Command::new("ffplay")
        .args(&["-nodisp", "-autoexit", &path])
        .spawn();

    #[cfg(target_os = "macos")]
    let result = Command::new("afplay").arg(&path).spawn();

    #[cfg(target_os = "windows")]
    let result = Command::new("powershell")
        .args([
            "-c",
            &format!("(New-Object Media.SoundPlayer '{}').PlaySync();", path),
        ])
        .spawn();

    match result {
        Ok(process) => {
            audio_state.current_process = Some(process);
            audio_state.current_song_path = Some(path);
            Ok(())
        }
        Err(e) => Err(format!("Failed to play audio: {}", e)),
    }
}

// Add a new command to check if audio is still playing
#[tauri::command]
fn is_audio_playing(state: tauri::State<Mutex<AudioState>>) -> Result<bool, String> {
    let mut audio_state = state.lock().map_err(|e| e.to_string())?;

    if let Some(process) = audio_state.current_process.as_mut() {
        // Check if the process is still running
        match process.try_wait() {
            Ok(Some(_)) => Ok(false), // Process has finished
            Ok(None) => Ok(true),     // Process is still running
            Err(_) => Ok(false),      // Error checking process
        }
    } else {
        Ok(false) // No process running
    }
}

#[tauri::command]
fn stop_audio(state: tauri::State<Mutex<AudioState>>) -> Result<(), String> {
    let mut audio_state = state.lock().map_err(|e| e.to_string())?;
    if let Some(mut process) = audio_state.current_process.take() {
        let _ = process.kill();
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(Mutex::new(AudioState::default()))
        .setup(|app| {
            // Resolve app data directory for the SQLite database
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");

            let db_path = app_data_dir.join("smartnotes.db");
            let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

            // Block on async DB setup during Tauri's synchronous setup hook
            let pool = tauri::async_runtime::block_on(async {
                let pool = sqlx::SqlitePool::connect(&db_url)
                    .await
                    .expect("failed to connect to SQLite database");

                // Enable WAL mode for better concurrent read performance
                sqlx::query("PRAGMA journal_mode=WAL;")
                    .execute(&pool)
                    .await
                    .expect("failed to set WAL mode");

                // Enable foreign key enforcement
                sqlx::query("PRAGMA foreign_keys=ON;")
                    .execute(&pool)
                    .await
                    .expect("failed to enable foreign keys");

                // Run migrations
                db::migrations::run_migrations(&pool)
                    .await
                    .expect("failed to run database migrations");

                pool
            });

            // Create the vault directory for .md notes
            let vault_path = app_data_dir.join("vault");
            std::fs::create_dir_all(&vault_path).expect("failed to create vault directory");
            migrate_legacy_editor_mirror(&app_data_dir)
                .expect("failed to migrate legacy mirrored editor files");

            let mirrored_editor_files = sync_editor_files_to_vault(&app_data_dir)
                .expect("failed to sync editor files into Smart Notes vault");
            if mirrored_editor_files > 0 {
                println!(
                    "[Smart Notes] Mirrored {} editor files into the vault",
                    mirrored_editor_files
                );
            }

            // Initialize the EmbedWorker
            let lance_db_path = app_data_dir.join("lancedb");
            let model_cache_path = app_data_dir.join("models");
            let embed_worker = tauri::async_runtime::block_on(async {
                rag::embedder::EmbedWorker::start(
                    pool.clone(),
                    lance_db_path.clone(),
                    model_cache_path.clone(),
                )
                .await
                .expect("failed to start embed worker")
            });

            println!("[Smart Notes] Embed worker started");

            // Initialize the NoteEngine with embed sender
            let mut note_engine = NoteEngine::new(pool.clone(), vault_path.clone());
            note_engine.set_embed_sender(embed_worker.sender.clone());

            // Run initial vault sync (index any existing .md files)
            let synced = tauri::async_runtime::block_on(async {
                note_engine
                    .sync_vault()
                    .await
                    .expect("failed to sync vault on startup")
            });
            println!(
                "[Smart Notes] Initial vault sync: {} notes processed",
                synced
            );

            // Start filesystem watcher
            let mut watcher_engine = NoteEngine::new(pool.clone(), vault_path.clone());
            watcher_engine.set_embed_sender(embed_worker.sender.clone());
            let engine_arc = Arc::new(watcher_engine);
            let watcher =
                NoteEngine::start_watcher(engine_arc).expect("failed to start filesystem watcher");

            // Initialize the SearchEngine (separate fastembed model for queries)
            let search_engine = tauri::async_runtime::block_on(async {
                rag::search::SearchEngine::new(lance_db_path.clone(), model_cache_path.clone())
                    .await
                    .expect("failed to start search engine")
            });
            println!("[Smart Notes] Search engine ready");

            // Initialize the HybridRetriever (vector + BM25 fusion)
            let hybrid_retriever = tauri::async_runtime::block_on(async {
                rag::retrieval::HybridRetriever::new(
                    lance_db_path.clone(),
                    model_cache_path.clone(),
                )
                .await
                .expect("failed to init hybrid retriever")
            });
            println!("[Smart Notes] Hybrid retriever ready");

            // Initialize Ollama backend
            let ollama = llm::ollama::OllamaBackend::new();
            println!("[Smart Notes] Ollama backend initialized (default model: llama3.2:3b)");

            let app_state = AppState {
                db: pool,
                note_engine,
                _watcher: Mutex::new(Some(watcher)),
                embed_worker,
                search_engine,
                hybrid_retriever: std::sync::Arc::new(hybrid_retriever),
                ollama: std::sync::Arc::new(ollama),
                conversations: tokio::sync::Mutex::new(std::collections::HashMap::new()),
            };
            app.manage(app_state);

            println!("[Smart Notes] Database initialized at {:?}", db_path);
            println!("[Smart Notes] Vault directory: {:?}", vault_path);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_user_folder,
            save_file,
            load_file,
            list_files,
            create_folder,
            delete_item,
            play_audio,
            stop_audio,
            is_audio_playing,
            commands::notes::get_all_notes,
            commands::notes::get_note,
            commands::notes::create_note,
            commands::notes::update_note,
            commands::notes::delete_note,
            commands::search::get_indexing_status,
            commands::search::reindex_all_notes,
            commands::search::semantic_search,
            commands::search::get_backlinks,
            commands::llm::check_ollama_status,
            commands::llm::list_ollama_models,
            commands::llm::set_ollama_model,
            commands::llm::set_ollama_completion_model,
            commands::rag::ask_notes,
            commands::search::get_related_notes,
            commands::import::import_vault,
            commands::suggestions::get_inline_suggestion
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
