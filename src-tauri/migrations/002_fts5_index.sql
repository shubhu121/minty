-- Smart Notes — FTS5 full-text search index for BM25 ranking
-- This virtual table mirrors chunk text for keyword search.

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    chunk_id UNINDEXED,
    note_id UNINDEXED,
    text,
    heading_path,
    content='chunks',
    content_rowid='rowid'
);

-- Triggers to keep FTS5 in sync with the chunks table

CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, chunk_id, note_id, text, heading_path)
    VALUES (new.rowid, new.id, new.note_id, new.text, new.heading_path);
END;

CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_id, note_id, text, heading_path)
    VALUES('delete', old.rowid, old.id, old.note_id, old.text, old.heading_path);
END;

CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_id, note_id, text, heading_path)
    VALUES('delete', old.rowid, old.id, old.note_id, old.text, old.heading_path);
    INSERT INTO chunks_fts(rowid, chunk_id, note_id, text, heading_path)
    VALUES (new.rowid, new.id, new.note_id, new.text, new.heading_path);
END;
