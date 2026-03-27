use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

fn walk_md_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk_md_files(&path, out)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn import_vault(
    state: State<'_, AppState>,
    source_path: String,
    _vault_type: String, // "obsidian" or "folder" (both handled by copying .md files recursively)
) -> Result<ImportResult, String> {
    let source_dir = Path::new(&source_path);
    if !source_dir.is_dir() {
        return Err("Source path is not a valid directory.".into());
    }

    let dest_dir = &state.note_engine.vault_path;

    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    let mut md_files = Vec::new();
    if let Err(e) = walk_md_files(source_dir, &mut md_files) {
        return Err(format!("Failed to read source directory: {}", e));
    }

    for path in md_files {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        let dest_path = dest_dir.join(&file_name);

        // Do not overwrite existing notes with the same filename. Skip them to prevent breaking wikilinks context arbitrarily.
        if dest_path.exists() {
            skipped += 1;
            continue;
        }

        match fs::copy(&path, &dest_path) {
            Ok(_) => {
                imported += 1;
            }
            Err(e) => {
                errors.push(format!("Failed to copy {}: {}", file_name, e));
            }
        }
    }

    // Trigger full re-index of imported notes
    if imported > 0 {
        if let Err(e) = state.note_engine.sync_vault().await {
            errors.push(format!("Failed to sync vault after import: {}", e));
        }
    }

    Ok(ImportResult {
        imported,
        skipped,
        errors,
    })
}
