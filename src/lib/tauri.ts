/**
 * Typed wrappers for all Tauri invoke() calls.
 * Every new Tauri command MUST have a corresponding wrapper here.
 */
import { invoke } from "@tauri-apps/api/core";


export interface NoteMetadata {
  id: string;
  path: string;
  title: string;
  word_count: number;
  created_at: number;
  updated_at: number;
}

export interface NoteContent {
  id: string;
  path: string;
  title: string;
  content: string;
  word_count: number;
  created_at: number;
  updated_at: number;
}

export interface RelatedNote {
  note_id: string;
  title: string;
  preview: string;
  similarity_score: number;
}

/** Fetch metadata for all notes in the vault. */
export async function getAllNotes(): Promise<NoteMetadata[]> {
  return invoke<NoteMetadata[]>("get_all_notes");
}

/** Fetch a single note with full content (read from disk). */
export async function getNote(id: string): Promise<NoteContent> {
  return invoke<NoteContent>("get_note", { id });
}

/** Create a new note with the given title. Returns the new note's metadata. */
export async function createNote(title: string): Promise<NoteMetadata> {
  return invoke<NoteMetadata>("create_note", { title });
}

/** Update a note's content. Writes to disk and updates the index. */
export async function updateNote(id: string, content: string): Promise<void> {
  return invoke<void>("update_note", { id, content });
}

/** Delete a note (moves to .trash, does not hard delete). */
export async function deleteNote(id: string): Promise<void> {
  return invoke<void>("delete_note", { id });
}


export interface IndexingStatus {
  total: number;
  indexed: number;
}

/** Get current indexing progress (total notes vs indexed notes). */
export async function getIndexingStatus(): Promise<{
  total: number;
  indexed: number;
}> {
  return invoke("get_indexing_status");
}

export async function getRelatedNotes(
  noteId: string,
  limit: number = 5
): Promise<RelatedNote[]> {
  return invoke<RelatedNote[]>("get_related_notes", { noteId, limit });
}

export interface ImportResult {
  imported: number;
  skipped: number;
  errors: string[];
}

export async function importVault(
  sourcePath: string,
  vaultType: "obsidian" | "folder"
): Promise<ImportResult> {
  return invoke<ImportResult>("import_vault", { sourcePath, vaultType });
}

export type SearchMode = "hybrid" | "semantic" | "keyword";

export interface RetrievedChunk {
  chunk_id: string;
  note_id: string;
  note_title: string;
  text: string;
  heading_path: string;
  char_start: number;
  char_end: number;
  rrf_score: number;
  vector_score: number;
  bm25_score: number;
}

export interface SearchResult {
  note_id: string;
  note_title: string;
  note_path: string;
  chunk_text: string;
  heading_path: string;
  char_start: number;
  char_end: number;
  score: number;
}

export async function searchNotes(
  query: string,
  limit?: number,
  mode?: SearchMode
): Promise<RetrievedChunk[]> {
  return invoke<RetrievedChunk[]>("semantic_search", { query, limit, mode });
}

export interface Backlink {
  source_id: string;
  source_title: string;
  source_path: string;
  anchor_text: string;
  link_type: string;
}

export async function getBacklinks(noteId: string): Promise<Backlink[]> {
  return invoke<Backlink[]>("get_backlinks", { noteId });
}


export interface OllamaStatus {
  available: boolean;
  models: string[];
  active_model: string;
  completion_model: string;
}

/** Check if Ollama is running and get available models. */
export async function checkOllamaStatus(): Promise<OllamaStatus> {
  return invoke<OllamaStatus>("check_ollama_status");
}

/** List all models available on the Ollama server. */
export async function listOllamaModels(): Promise<string[]> {
  return invoke<string[]>("list_ollama_models");
}

/** Set the active Ollama model. */
export async function setOllamaModel(model: string): Promise<void> {
  return invoke<void>("set_ollama_model", { model });
}

/** Set the active Ollama completion model. */
export async function setOllamaCompletionModel(model: string): Promise<void> {
  return invoke<void>("set_ollama_completion_model", { model });
}


export interface WritingFile {
  name: string;
  text: string;
  font: string;
  font_size: number;
  theme: string;
}

export async function getUserFolder(): Promise<string> {
  return invoke<string>("get_user_folder");
}

export async function saveFile(file: WritingFile): Promise<string> {
  return invoke<string>("save_file", { file });
}

export async function loadFile(name: string): Promise<WritingFile> {
  return invoke<WritingFile>("load_file", { name });
}

export async function listFiles(): Promise<string[]> {
  return invoke<string[]>("list_files");
}

export async function createFolder(name: string): Promise<string> {
  return invoke<string>("create_folder", { name });
}

export async function deleteItem(name: string): Promise<string> {
  return invoke<string>("delete_item", { name });
}

export async function playAudio(path: string): Promise<void> {
  return invoke<void>("play_audio", { path });
}

export async function stopAudio(): Promise<void> {
  return invoke<void>("stop_audio");
}

export async function isAudioPlaying(): Promise<boolean> {
  return invoke<boolean>("is_audio_playing");
}
