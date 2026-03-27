-- ============================================================
-- Smart Notes — Canonical Schema Reference
-- ============================================================
-- This file is NOT used at runtime. It serves as documentation
-- of the current database schema. Actual migrations live in
-- src-tauri/migrations/*.sql
-- ============================================================

-- Notes metadata (content lives in .md files on disk)
CREATE TABLE notes (
    id TEXT PRIMARY KEY,              -- UUID v4
    path TEXT UNIQUE NOT NULL,        -- relative path within vault dir
    title TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '', -- SHA-256 hex of file content
    word_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,      -- unix timestamp
    updated_at INTEGER NOT NULL       -- unix timestamp
);

-- Embedding chunks for RAG
CREATE TABLE chunks (
    id TEXT PRIMARY KEY,              -- UUID v4
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    text TEXT NOT NULL,
    char_start INTEGER NOT NULL,
    char_end INTEGER NOT NULL,
    heading_path TEXT NOT NULL DEFAULT '',   -- e.g. "Introduction > Background"
    embedding_model TEXT NOT NULL DEFAULT '',
    indexed_at INTEGER NOT NULL,      -- unix timestamp
    UNIQUE(note_id, chunk_index)
);

-- Wikilinks between notes
CREATE TABLE links (
    source_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    link_type TEXT NOT NULL DEFAULT 'explicit',
    anchor_text TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (source_id, target_id, link_type)
);

-- Tags
CREATE TABLE tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);

-- App settings (key-value)
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Indices
CREATE INDEX idx_chunks_note_id ON chunks(note_id);
CREATE INDEX idx_links_source   ON links(source_id);
CREATE INDEX idx_links_target   ON links(target_id);
CREATE INDEX idx_tags_note      ON tags(note_id);
