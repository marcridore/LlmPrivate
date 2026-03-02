# LlmPrivate

A privacy-focused desktop application for running local LLMs. All inference happens on your machine — no data leaves your computer.

Built with **Tauri v2** (Rust backend) + **React / TypeScript / Tailwind** (frontend) + **llama.cpp** (inference).

## Features

- **Chat** — Conversational interface with local LLM models (text + vision)
- **Chat with Documents** — Add PDFs, TXT, MD, DOCX files, then chat, summarize, or quiz against them using RAG (FTS5 full-text search retrieval)
- **Model Browser** — Download recommended GGUF models from HuggingFace, or scan local files
- **Vision / Multimodal** — SmolVLM2 support via llama-server sidecar for image understanding
- **Backup & Restore** — Export all data to a ZIP file and restore on another machine
- **GPU Accelerated** — CUDA and Vulkan support for fast inference

## Prerequisites

- **Node.js** >= 18
- **Rust** (latest stable via [rustup](https://rustup.rs))
- **Visual Studio 2022/2025** with C++ build tools (Windows)
- **CUDA Toolkit 13.1** (for GPU acceleration — optional)
- **Ninja** build system (required for CUDA builds with VS 2025)

Install Ninja via winget:
```
winget install Ninja-build.Ninja
```

## Development

### Quick start (CPU only)

```bash
npm install
npm run tauri dev
```

### With CUDA GPU acceleration (recommended)

VS 2025 (v18) requires special flags because nvcc doesn't officially support it as a host compiler:

```bash
npm install

# Windows (Git Bash / MSYS2):
PATH="/c/Users/<you>/AppData/Local/Microsoft/WinGet/Packages/Ninja-build.Ninja_Microsoft.Winget.Source_8wekyb3d8bbwe:${PATH}" \
CMAKE_GENERATOR=Ninja \
CUDAFLAGS="-allow-unsupported-compiler" \
npm run tauri dev -- --features cuda
```

Replace `<you>` with your Windows username.

**What happens on first build:**
- `llama-cpp-sys-2` compiles llama.cpp with CUDA — this takes **10-35 minutes** the first time (600+ CUDA kernels compiled by ptxas/cicc)
- Subsequent builds only recompile changed Rust code (~15-30 seconds)
- `cargo check` (type-checking only) takes ~15-30 seconds even after clean

### Rust-only check (fast)

To verify Rust code compiles without building the full binary:

```bash
# With CUDA
PATH="...(ninja path)..." CMAKE_GENERATOR=Ninja CUDAFLAGS="-allow-unsupported-compiler" \
cargo check --manifest-path src-tauri/Cargo.toml --features cuda

# Without CUDA
cargo check --manifest-path src-tauri/Cargo.toml
```

### Running tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## Production Build

```bash
# CPU only
npm run tauri build

# With CUDA
PATH="...(ninja path)..." CMAKE_GENERATOR=Ninja CUDAFLAGS="-allow-unsupported-compiler" \
npm run tauri build -- --features cuda
```

This produces an NSIS installer at:
```
src-tauri/target/release/bundle/nsis/LlmPrivate_0.1.0_x64-setup.exe
```

## Project Structure

```
LlmPrivate/
├── src/                          # Frontend (React + TypeScript)
│   ├── components/
│   │   ├── chat/                 # Chat UI, message bubbles, input
│   │   ├── documents/            # Document management, folder tree, doc chat
│   │   ├── layout/               # App shell, sidebar, title bar, status bar
│   │   ├── models/               # Model browser, download manager
│   │   ├── settings/             # Settings page (backup/restore)
│   │   └── ui/                   # Shared UI components
│   ├── stores/                   # Zustand state stores
│   └── types/                    # TypeScript type definitions
│
├── src-tauri/                    # Backend (Rust + Tauri v2)
│   ├── src/
│   │   ├── backend/              # LLM inference (llama-cpp-2, vision server)
│   │   ├── commands/             # Tauri IPC command handlers
│   │   │   ├── backup.rs         # Export/import backup
│   │   │   ├── chat.rs           # Send message, stop generation
│   │   │   ├── documents.rs      # Add/delete docs, doc chat, search
│   │   │   ├── models.rs         # Load/unload/download models
│   │   │   ├── settings.rs       # Key-value settings
│   │   │   └── system.rs         # System info, GPU detection
│   │   ├── db/                   # SQLite database (rusqlite)
│   │   │   └── connection.rs     # Schema, migrations, all queries
│   │   ├── documents/            # Document processing pipeline
│   │   │   ├── chunker.rs        # Text chunking with overlap
│   │   │   ├── parser.rs         # PDF/TXT/MD/DOCX text extraction
│   │   │   ├── retriever.rs      # FTS5 search + context building
│   │   │   └── types.rs          # Document data types
│   │   ├── models/               # Model management + downloads
│   │   ├── lib.rs                # Tauri app setup, command registration
│   │   └── state.rs              # AppState (shared across commands)
│   └── Cargo.toml
│
├── package.json
└── README.md
```

## Data Storage

All app data is stored under the Tauri app data directory:

| Platform | Path |
|----------|------|
| Windows  | `%APPDATA%/com.llmprivate.app/` |
| macOS    | `~/Library/Application Support/com.llmprivate.app/` |
| Linux    | `~/.local/share/com.llmprivate.app/` |

```
com.llmprivate.app/
├── llmprivate.db        # SQLite database (conversations, documents, settings)
├── llmprivate.log       # Application log file
├── documents/           # Imported document copies (UUID-named)
├── models/              # Downloaded GGUF model files
└── bin/
    └── llama-server/    # Vision model sidecar binary
```

## Backup & Restore

LlmPrivate can export all your data (conversations, documents, folder structure, settings) to a single ZIP file for transfer to another machine.

### Export

1. Go to **Settings** (gear icon in sidebar)
2. Click **Export Backup**
3. Choose where to save the `.zip` file
4. Wait for the progress to complete

The backup includes:
- SQLite database (clean snapshot via `VACUUM INTO`)
- All imported document files
- A manifest with metadata for path rewriting

**Not included:** Model files (1-4 GB each — re-download them on the new machine).

### Import / Restore

1. Go to **Settings**
2. Click **Import Backup**
3. Select a `.zip` backup file
4. Confirm the warning (restore **replaces** all current data)
5. Restart the app after restore completes

The restore automatically rewrites document file paths to match the new machine's data directory.

## Architecture Notes

### Inference

- **Text models** — In-process via `llama-cpp-2` (Rust bindings to llama.cpp). Fast, no HTTP overhead.
- **Vision models** — `llama-server` sidecar process exposing an OpenAI-compatible API on port 8990. Required because the in-process llama-cpp-2 has an mmproj loading bug with certain vision models.

### Document Chat (RAG)

1. **Parse** — Extract text from PDF/TXT/MD/DOCX
2. **Chunk** — Split into ~500-char overlapping segments with sentence-boundary detection
3. **Index** — Store in SQLite FTS5 for full-text search with BM25 ranking
4. **Retrieve** — On chat: FTS5 search for relevant chunks. On summarize/quiz: sequential chunks.
5. **Generate** — Inject retrieved context into system prompt, stream LLM response

### Database

SQLite with 9 tables + FTS5 virtual table. Migrations run automatically on startup. Key tables:
- `conversations` / `messages` — Chat history
- `doc_folders` / `documents` / `document_chunks` / `chunks_fts` — Document storage + search index
- `doc_chat_sessions` / `doc_chat_documents` — Document chat history with pinning
- `settings` — Key-value application settings

## License

Private — all rights reserved.
