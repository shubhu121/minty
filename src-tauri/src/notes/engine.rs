use anyhow::{Context, Result};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::parser;
use crate::rag::embedder::EmbedJob;
use crate::rag::lang;

/// Metadata about a note, returned to the frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteMetadata {
    pub id: String,
    pub path: String,
    pub title: String,
    pub word_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Full note content, returned when loading a specific note.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteContent {
    pub id: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub word_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The core note engine managing .md files on disk with SQLite as metadata index.
pub struct NoteEngine {
    pub db: SqlitePool,
    pub vault_path: PathBuf,
    /// Optional sender to queue embedding jobs.
    pub embed_sender: Option<mpsc::Sender<EmbedJob>>,
}

impl NoteEngine {
    pub fn new(db: SqlitePool, vault_path: PathBuf) -> Self {
        Self {
            db,
            vault_path,
            embed_sender: None,
        }
    }

    /// Set the embed sender for queuing embedding jobs.
    pub fn set_embed_sender(&mut self, sender: mpsc::Sender<EmbedJob>) {
        self.embed_sender = Some(sender);
    }

    /// Queue an embed job if the sender is available.
    pub fn queue_embed(&self, note_id: &str, note_path: &str, content: &str) {
        if let Some(sender) = &self.embed_sender {
            let job = EmbedJob {
                note_id: note_id.to_string(),
                note_path: note_path.to_string(),
                content: content.to_string(),
            };
            let sender = sender.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = sender.send(job).await {
                    eprintln!("[NoteEngine] Failed to queue embed job: {}", e);
                }
            });
        }
    }

    /// Walk the vault directory, hash each .md file, and upsert changed notes into SQLite.
    pub async fn sync_vault(&self) -> Result<usize> {
        let vault = self.vault_path.clone();
        let entries = tokio::task::spawn_blocking(move || -> Result<Vec<(PathBuf, String)>> {
            let mut files = Vec::new();
            walk_md_files(&vault, &mut files)?;
            Ok(files
                .into_iter()
                .map(|p| {
                    let content = std::fs::read_to_string(&p).unwrap_or_default();
                    (p, content)
                })
                .collect())
        })
        .await??;

        let mut synced = 0;
        for (path, content) in entries {
            let rel_path = path
                .strip_prefix(&self.vault_path)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            let content_hash = hash_content(&content);

            // Check if this note already exists and if hash changed
            let existing = sqlx::query_as::<_, (String, String, i64)>(
                r#"
                SELECT
                    n.id,
                    n.content_hash,
                    EXISTS(SELECT 1 FROM chunks c WHERE c.note_id = n.id) AS has_chunks
                FROM notes n
                WHERE n.path = ?
                "#,
            )
            .bind(&rel_path)
            .fetch_optional(&self.db)
            .await?;

            match existing {
                Some((id, existing_hash, has_chunks)) => {
                    let needs_metadata_update = existing_hash != content_hash;
                    let needs_index = needs_metadata_update || has_chunks == 0;

                    if needs_metadata_update {
                        // Content changed — re-parse and update
                        let parsed = parser::parse_note(&content);
                        let lang = lang::detect_language(&content)
                            .map(|info| info.lang)
                            .unwrap_or_else(|| "Unknown".to_string());

                        let now = chrono::Utc::now().timestamp();
                        let word_count = content.split_whitespace().count() as i64;
                        let content_hash = hash_content(&content);

                        sqlx::query(
                            "UPDATE notes SET title = ?, content_hash = ?, word_count = ?, updated_at = ?, lang = ? WHERE id = ?"
                        )
                        .bind(&parsed.title)
                        .bind(&content_hash)
                        .bind(word_count)
                        .bind(now)
                        .bind(&lang)
                        .bind(&id)
                        .execute(&self.db)
                        .await?;

                        // Update links
                        self.update_links(&id, &parsed.wikilinks).await?;

                        synced += 1;
                    }

                    if needs_index {
                        self.queue_embed(&id, &rel_path, &content);
                        if !needs_metadata_update {
                            synced += 1;
                        }
                    }
                }
                None => {
                    // New note — insert
                    let parsed = parser::parse_note(&content);
                    let now = chrono::Utc::now().timestamp();
                    let id = Uuid::new_v4().to_string();

                    let lang = lang::detect_language(&content)
                        .map(|info| info.lang)
                        .unwrap_or_else(|| "Unknown".to_string());

                    sqlx::query(
                        "INSERT INTO notes (id, path, title, content_hash, word_count, created_at, updated_at, lang) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(&id)
                    .bind(&rel_path)
                    .bind(&parsed.title)
                    .bind(&content_hash)
                    .bind(parsed.word_count as i64)
                    .bind(now)
                    .bind(now)
                    .bind(&lang)
                    .execute(&self.db)
                    .await?;

                    // Insert links
                    self.update_links(&id, &parsed.wikilinks).await?;

                    // Queue initial embedding for notes discovered during
                    // startup syncs, imports, and watcher-driven syncs.
                    self.queue_embed(&id, &rel_path, &content);

                    synced += 1;
                }
            }
        }

        // Remove notes from DB whose files no longer exist
        let db_notes: Vec<(String, String)> = sqlx::query_as("SELECT id, path FROM notes")
            .fetch_all(&self.db)
            .await?;

        for (id, rel_path) in db_notes {
            let full_path = self.vault_path.join(&rel_path);
            if !full_path.exists() {
                sqlx::query("DELETE FROM notes WHERE id = ?")
                    .bind(&id)
                    .execute(&self.db)
                    .await?;
                synced += 1;
            }
        }

        Ok(synced)
    }

    /// Update wikilinks for a note (delete old, insert new).
    async fn update_links(&self, source_id: &str, wikilinks: &[parser::WikiLink]) -> Result<()> {
        // Remove old links from this source
        sqlx::query("DELETE FROM links WHERE source_id = ?")
            .bind(source_id)
            .execute(&self.db)
            .await?;

        // Try to resolve each wikilink to a note ID
        for wl in wikilinks {
            // Try to find target note by path (with or without .md extension)
            let target_path_md = format!("{}.md", wl.target);
            let target = sqlx::query_as::<_, (String,)>(
                "SELECT id FROM notes WHERE path = ? OR path = ? OR title = ?",
            )
            .bind(&wl.target)
            .bind(&target_path_md)
            .bind(&wl.target)
            .fetch_optional(&self.db)
            .await?;

            if let Some((target_id,)) = target {
                let anchor = wl.alias.as_deref().unwrap_or(&wl.target);
                sqlx::query(
                    "INSERT OR IGNORE INTO links (source_id, target_id, link_type, anchor_text) VALUES (?, ?, 'explicit', ?)"
                )
                .bind(source_id)
                .bind(&target_id)
                .bind(anchor)
                .execute(&self.db)
                .await?;
            }
        }

        Ok(())
    }

    /// Get all notes metadata.
    pub async fn get_all_notes(&self) -> Result<Vec<NoteMetadata>> {
        let notes = sqlx::query_as::<_, (String, String, String, i64, i64, i64)>(
            "SELECT id, path, title, word_count, created_at, updated_at FROM notes ORDER BY updated_at DESC"
        )
        .fetch_all(&self.db)
        .await?;

        Ok(notes
            .into_iter()
            .map(
                |(id, path, title, word_count, created_at, updated_at)| NoteMetadata {
                    id,
                    path,
                    title,
                    word_count,
                    created_at,
                    updated_at,
                },
            )
            .collect())
    }

    /// Get a single note with full content (read from disk).
    pub async fn get_note(&self, id: &str) -> Result<NoteContent> {
        let row = sqlx::query_as::<_, (String, String, String, i64, i64, i64)>(
            "SELECT id, path, title, word_count, created_at, updated_at FROM notes WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?
        .context("Note not found")?;

        let full_path = self.vault_path.join(&row.1);
        let content = tokio::fs::read_to_string(&full_path)
            .await
            .context("Failed to read note file from disk")?;

        Ok(NoteContent {
            id: row.0,
            path: row.1,
            title: row.2,
            content,
            word_count: row.3,
            created_at: row.4,
            updated_at: row.5,
        })
    }

    /// Create a new note. Creates the .md file on disk and inserts metadata into SQLite.
    pub async fn create_note(&self, title: &str) -> Result<NoteMetadata> {
        let filename = sanitize_filename(title);
        let rel_path = format!("{}.md", filename);
        let full_path = self.vault_path.join(&rel_path);

        // Ensure we don't overwrite
        let full_path = ensure_unique_path(full_path);
        let rel_path = full_path
            .strip_prefix(&self.vault_path)
            .unwrap_or(&full_path)
            .to_string_lossy()
            .replace('\\', "/");

        // Create initial content with title as H1
        let initial_content = format!("# {}\n\n", title);
        tokio::fs::write(&full_path, &initial_content).await?;

        let now = chrono::Utc::now().timestamp();
        let id = Uuid::new_v4().to_string();
        let content_hash = hash_content(&initial_content);
        let word_count = initial_content.split_whitespace().count() as i64;
        let lang = lang::detect_language(&initial_content)
            .map(|l| l.lang)
            .unwrap_or_else(|| "Unknown".to_string());

        sqlx::query(
            "INSERT INTO notes (id, path, title, content_hash, word_count, created_at, updated_at, lang) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&rel_path)
        .bind(title)
        .bind(&content_hash)
        .bind(word_count)
        .bind(now)
        .bind(now)
        .bind(&lang)
        .execute(&self.db)
        .await?;

        // Queue embedding job
        self.queue_embed(&id, &rel_path, &initial_content);

        Ok(NoteMetadata {
            id,
            path: rel_path,
            title: title.to_string(),
            word_count,
            created_at: now,
            updated_at: now,
        })
    }

    /// Update a note's content. Writes to disk and updates SQLite metadata.
    pub async fn update_note(&self, id: &str, content: &str) -> Result<()> {
        let row = sqlx::query_as::<_, (String,)>("SELECT path FROM notes WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .context("Note not found")?;

        let full_path = self.vault_path.join(&row.0);
        tokio::fs::write(&full_path, content).await?;

        let parsed = parser::parse_note(content);
        let content_hash = hash_content(content);
        let now = chrono::Utc::now().timestamp();

        let lang = lang::detect_language(content)
            .map(|l| l.lang)
            .unwrap_or_else(|| "Unknown".to_string());

        sqlx::query(
            "UPDATE notes SET title = ?, content_hash = ?, word_count = ?, updated_at = ?, lang = ? WHERE id = ?"
        )
        .bind(&parsed.title)
        .bind(&content_hash)
        .bind(parsed.word_count as i64)
        .bind(now)
        .bind(&lang)
        .bind(id)
        .execute(&self.db)
        .await?;

        self.update_links(id, &parsed.wikilinks).await?;

        // Queue embedding job
        self.queue_embed(id, &row.0, content);

        Ok(())
    }

    /// Delete a note — moves the file to a .trash folder instead of hard delete.
    pub async fn delete_note(&self, id: &str) -> Result<()> {
        let row = sqlx::query_as::<_, (String,)>("SELECT path FROM notes WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .context("Note not found")?;

        let full_path = self.vault_path.join(&row.0);

        // Move to .trash instead of hard delete
        let trash_dir = self.vault_path.join(".trash");
        tokio::fs::create_dir_all(&trash_dir).await?;

        let trash_path = trash_dir.join(full_path.file_name().unwrap_or_default());
        let trash_path = ensure_unique_path(trash_path);

        if full_path.exists() {
            tokio::fs::rename(&full_path, &trash_path).await?;
        }

        // Remove from SQLite (CASCADE will clean up links, tags, chunks)
        sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    /// Start the filesystem watcher. Returns the watcher handle (must be kept alive).
    pub fn start_watcher(engine: Arc<NoteEngine>) -> Result<RecommendedWatcher> {
        let vault_path = engine.vault_path.clone();

        let (tx, mut rx) = mpsc::channel::<()>(16);

        // Debounced event handler: sends a signal to the channel
        let watcher_tx = tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                if let Ok(event) = result {
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                            // Check if any of the paths are .md files
                            let is_md = event.paths.iter().any(|p| {
                                p.extension()
                                    .map(|e| e == "md")
                                    .unwrap_or(false)
                                    // Ignore .trash directory
                                    && !p.to_string_lossy().contains(".trash")
                            });
                            if is_md {
                                let _ = watcher_tx.try_send(());
                            }
                        }
                        _ => {}
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )?;

        watcher.watch(vault_path.as_ref(), RecursiveMode::Recursive)?;

        // Spawn a background task that debounces and syncs
        let engine_clone = engine.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                // Wait for at least one event
                if rx.recv().await.is_none() {
                    break; // Channel closed
                }

                // Debounce: drain any additional events for 500ms
                tokio::time::sleep(Duration::from_millis(500)).await;
                while rx.try_recv().is_ok() {}

                // Sync
                match engine_clone.sync_vault().await {
                    Ok(n) => {
                        if n > 0 {
                            println!("[Smart Notes] Vault sync: {} notes updated", n);
                        }
                    }
                    Err(e) => {
                        eprintln!("[Smart Notes] Vault sync error: {}", e);
                    }
                }
            }
        });

        Ok(watcher)
    }
}

/// Recursively walk a directory for .md files, skipping hidden dirs and .trash.
fn walk_md_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        // Skip hidden directories and .trash
        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            walk_md_files(&path, out)?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            out.push(path);
        }
    }

    Ok(())
}

fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Sanitize a filename: replace invalid chars with hyphens, trim.
fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim().to_string();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed
    }
}

/// Ensure a file path is unique by appending (1), (2), etc. if needed.
fn ensure_unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }

    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or(Path::new("."));

    let mut counter = 1;
    loop {
        let new_name = format!("{}({}){}", stem, counter, ext);
        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}
