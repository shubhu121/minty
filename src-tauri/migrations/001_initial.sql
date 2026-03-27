-- Smart Notes — Initial Schema
-- Notes metadata (filesystem .md files are source of truth for content)
CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '',
    word_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Embedding chunks for RAG pipeline
CREATE TABLE IF NOT EXISTS chunks (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    text TEXT NOT NULL,
    char_start INTEGER NOT NULL,
    char_end INTEGER NOT NULL,
    heading_path TEXT NOT NULL DEFAULT '',
    embedding_model TEXT NOT NULL DEFAULT '',
    indexed_at INTEGER NOT NULL,
    UNIQUE(note_id, chunk_index)
);

-- Wikilinks and backlinks between notes
CREATE TABLE IF NOT EXISTS links (
    source_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    link_type TEXT NOT NULL DEFAULT 'explicit',
    anchor_text TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (source_id, target_id, link_type)
);

-- Note tags
CREATE TABLE IF NOT EXISTS tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);

-- Key-value settings store
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Performance indices
CREATE INDEX IF NOT EXISTS idx_chunks_note_id ON chunks(note_id);
CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_id);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_id);
CREATE INDEX IF NOT EXISTS idx_tags_note ON tags(note_id);
