# minty

minty is a local-first desktop writing app built with Tauri, React, and Rust. It combines a distraction-free editor with note indexing, semantic search, backlink-aware retrieval, local Ollama chat, and inline completion.

## What the app does

- Provides a simple writing surface powered by Lexical on the frontend.
- Saves editor files locally as JSON session files.
- Mirrors saved editor files into a Markdown vault so Smart Notes features can index them.
- Indexes notes into SQLite, FTS5, and LanceDB for keyword, semantic, and hybrid retrieval.
- Uses Ollama locally for retrieval-augmented chat and inline suggestions.

## Tech stack

- Frontend: React 19, Vite, TypeScript, Lexical, Tailwind CSS
- Desktop shell: Tauri 2
- Backend: Rust
- Metadata store: SQLite via SQLx
- Vector store: LanceDB
- Full-text search: SQLite FTS5
- Embeddings: fastembed with `intfloat/multilingual-e5-small`
- Local LLM runtime: Ollama

## Architecture

### Frontend

The frontend lives in [`src`](/d:/writeSimply/src) and is responsible for editor UX, overlays, search UI, AI settings, RAG chat, and inline suggestions.

Important frontend files:

- [`src/App.tsx`](/d:/writeSimply/src/App.tsx): top-level app shell and editor workflow
- [`src/components/LexicalEditor.tsx`](/d:/writeSimply/src/components/LexicalEditor.tsx): main editor component
- [`src/components/SmartSearch.tsx`](/d:/writeSimply/src/components/SmartSearch.tsx): search UI
- [`src/components/AiSettingsPanel.tsx`](/d:/writeSimply/src/components/AiSettingsPanel.tsx): Ollama settings and vault import
- [`src/components/RagChat`](/d:/writeSimply/src/components/RagChat): retrieval-augmented chat UI
- [`src/lib/tauri.ts`](/d:/writeSimply/src/lib/tauri.ts): typed wrappers around Tauri commands

### Tauri backend

The Rust backend lives in [`src-tauri/src`](/d:/writeSimply/src-tauri/src).

Core backend responsibilities:

- expose Tauri commands
- manage app startup and shared state
- sync Markdown notes from disk into SQLite
- generate embeddings in the background
- serve vector, keyword, and hybrid retrieval
- stream chat and completion output from Ollama

Important backend files:

- [`src-tauri/src/lib.rs`](/d:/writeSimply/src-tauri/src/lib.rs): app startup, shared setup, file persistence, editor-to-vault mirroring
- [`src-tauri/src/state.rs`](/d:/writeSimply/src-tauri/src/state.rs): shared app state
- [`src-tauri/src/commands`](/d:/writeSimply/src-tauri/src/commands): Tauri command handlers
- [`src-tauri/src/notes/engine.rs`](/d:/writeSimply/src-tauri/src/notes/engine.rs): vault sync, note CRUD, watcher integration
- [`src-tauri/src/rag/embedder.rs`](/d:/writeSimply/src-tauri/src/rag/embedder.rs): background embedding worker
- [`src-tauri/src/rag/search.rs`](/d:/writeSimply/src-tauri/src/rag/search.rs): vector search
- [`src-tauri/src/rag/retrieval.rs`](/d:/writeSimply/src-tauri/src/rag/retrieval.rs): hybrid retrieval and BM25 fusion
- [`src-tauri/src/rag/generation.rs`](/d:/writeSimply/src-tauri/src/rag/generation.rs): RAG orchestration and streaming citations
- [`src-tauri/src/llm/ollama.rs`](/d:/writeSimply/src-tauri/src/llm/ollama.rs): Ollama client implementation

## Storage model

minty uses multiple local storage layers, each with a different role.

### 1. Editor session files

Saved editor files are stored as JSON under the app data directory:

- `AppData/Roaming/com.positronx.writesimply/user_data` on Windows

These files preserve editor-specific information such as:

- file name
- text
- font
- font size
- theme

### 2. Mirrored Markdown vault

To make editor files searchable by Smart Notes, the backend mirrors saved editor files into:

- `vault/minty/*.md`

On startup, the backend also migrates older mirrored files from:

- `vault/writesimply`

This keeps existing local content working after the rename.

### 3. SQLite database

SQLite stores structured metadata:

- notes
- chunks
- links
- tags
- settings

Schema and migrations:

- [`src-tauri/src/db/schema.sql`](/d:/writeSimply/src-tauri/src/db/schema.sql)
- [`src-tauri/migrations`](/d:/writeSimply/src-tauri/migrations)

### 4. FTS5 index

SQLite FTS5 mirrors chunk text for BM25 ranking and keyword search.

### 5. LanceDB

LanceDB stores embedding vectors for semantic and hybrid retrieval.

## Indexing pipeline

The indexing flow is:

1. A file is saved from the editor.
2. The backend writes the JSON session file into `user_data`.
3. The backend mirrors the same text into `vault/minty/*.md`.
4. `NoteEngine` syncs the vault into the `notes` table.
5. `EmbedWorker` chunks note text and writes:
   - structured chunks into SQLite
   - FTS rows through triggers
   - vectors into LanceDB
6. Search and RAG read from these indexed stores.

This means local AI features depend on indexed note chunks, not only on Ollama being available.

## Search modes

The app currently supports three retrieval modes:

- Keyword: FTS5 BM25 over chunk text
- Semantic: embedding search over LanceDB
- Hybrid: reciprocal rank fusion across vector and BM25 results

Hybrid retrieval is used by local chat so that natural-language and exact-term matches both contribute to context selection.

## Local setup

### Prerequisites

- Node.js 20 or newer
- npm
- Rust stable toolchain
- Tauri system dependencies
- Ollama for AI features

Linux system packages used in CI are listed in:

- [`/.github/workflows/appos-build.yml`](/d:/writeSimply/.github/workflows/appos-build.yml)

### Install dependencies

```bash
npm install
```

### Run in development

```bash
npm run tauri dev
```

This starts:

- the Vite dev server
- the Tauri desktop process
- the Rust backend watcher

### Build for production

```bash
npm run tauri build
```

## Ollama setup

Ollama is optional for plain editing, but required for:

- RAG chat
- inline suggestions
- AI-assisted note workflows

Start Ollama locally:

```bash
ollama serve
```

Pull the default models used by the app:

```bash
ollama pull llama3.2:3b
ollama pull qwen2.5:1.5b
```

Current defaults in the backend:

- chat model: `llama3.2:3b`
- completion model: `qwen2.5:1.5b`

See:

- [`src-tauri/src/llm/ollama.rs`](/d:/writeSimply/src-tauri/src/llm/ollama.rs)

## Common workflows

### Import an existing vault

Use the AI settings panel to import a Markdown folder or Obsidian vault. Imported `.md` files are copied into the app vault and then synced for indexing.

Relevant backend command:

- [`src-tauri/src/commands/import.rs`](/d:/writeSimply/src-tauri/src/commands/import.rs)

### Search notes

Use Smart Search in the UI. The frontend calls the Tauri search command wrappers in [`src/lib/tauri.ts`](/d:/writeSimply/src/lib/tauri.ts), which route to backend retrieval commands.

### Ask questions over notes

RAG chat retrieves chunks, assembles context, streams tokens, and emits citation events to the frontend.

## Project layout

```text
.
|-- public/
|-- src/
|   |-- components/
|   |-- hooks/
|   |-- lib/
|   `-- store/
|-- src-tauri/
|   |-- migrations/
|   |-- src/
|   |   |-- commands/
|   |   |-- db/
|   |   |-- llm/
|   |   |-- notes/
|   |   |-- rag/
|   |   `-- state.rs
|   `-- tauri.conf.json
|-- package.json
`-- README.md
```

## Troubleshooting

### Indexing stays at 0%

Check these first:

- notes exist in the mirrored vault under `vault/minty`
- the startup log shows vault sync activity
- the embed worker starts successfully
- the `chunks` table is being populated

If Ollama is connected but indexing is still 0%, the problem is usually in the note sync or embedding pipeline, not in Ollama.

### Search returns no results

Possible causes:

- notes were never mirrored into the Markdown vault
- notes exist in SQLite but have no chunk rows
- embeddings were not written to LanceDB
- FTS rows are empty or out of sync

### Hybrid or BM25 retrieval errors

Check:

- FTS migrations are applied
- `chunks` and `chunks_fts` are populated
- BM25 queries align with the external content table schema

## Notes for contributors

- Keep frontend changes inside the existing React and Tauri command boundaries.
- Prefer typed wrappers in [`src/lib/tauri.ts`](/d:/writeSimply/src/lib/tauri.ts) for every Tauri command.
- When changing note storage or indexing behavior, verify the full path:
  save -> mirror -> sync -> chunk -> FTS -> LanceDB -> retrieval
- Be careful with the Tauri bundle identifier. It controls the app data directory and should not be changed casually.

## License

This project is licensed under the MIT License. See [`LICENSE`](/d:/writeSimply/LICENSE).
