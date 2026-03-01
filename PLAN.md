# NexusLM - Local LLM Platform (Full Architecture Plan)

## Project Name: **NexusLM**
> A privacy-first, local AI platform that goes beyond chat — agents, RAG, multi-model workflows, and a developer API, all running on your machine.

---

## 1. WHY THIS IS BETTER THAN OLLAMA / LM STUDIO

| Feature | Ollama | LM Studio | **NexusLM** |
|---|---|---|---|
| Chat UI | No (CLI) | Yes (basic) | **Rich UI + multi-tab conversations** |
| Agent/Tool-use | No | No | **Built-in MCP + function calling** |
| Local RAG | No | No | **Drag-drop docs, auto-index, cite sources** |
| Multi-model workflows | No | No | **Route/chain/ensemble models** |
| OpenAI-compatible API | Yes (basic) | Yes (basic) | **Full API: batching, streaming, tools** |
| Auto-updates | No | Yes | **Tauri updater + changelog notifications** |
| Push notifications | No | No | **System tray + toast notifications** |
| Model Hub | CLI pull | In-app browse | **HuggingFace browser + 1-click download** |
| GPU detection | Manual | Auto | **Auto-detect + recommend models** |
| Plugin system | No | No | **MCP-based extensible tools** |

---

## 2. TECH STACK

```
┌─────────────────────────────────────────────────┐
│  Frontend: React 19 + TypeScript + Vite          │
│  UI: Tailwind CSS + shadcn/ui + Radix            │
│  State: Zustand   Markdown: react-markdown       │
│  Code: Shiki (syntax highlight)                  │
└─────────────────────┬───────────────────────────┘
                      │ Tauri IPC (invoke/listen)
┌─────────────────────▼───────────────────────────┐
│  Tauri 2.x Backend (Rust)                        │
│  ├─ llama-cpp-2 (Rust bindings to llama.cpp)     │
│  ├─ LanceDB (embedded vector store)              │
│  ├─ SQLite via rusqlite (conversations/settings)  │
│  ├─ axum (local HTTP API server)                  │
│  ├─ tokio (async runtime)                         │
│  ├─ reqwest (HuggingFace API for model downloads) │
│  ├─ lopdf + docx-rs (document parsing)            │
│  ├─ notify-rust (system notifications)            │
│  └─ tauri-plugin-updater (auto-updates)           │
└─────────────────────────────────────────────────┘
```

---

## 3. PROJECT STRUCTURE

```
nexuslm/
├── src-tauri/                     # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs                # Entry point, Tauri setup
│   │   ├── lib.rs                 # Module declarations
│   │   ├── state.rs               # App state (Arc<Mutex<...>>)
│   │   │
│   │   ├── inference/             # LLM inference engine
│   │   │   ├── mod.rs
│   │   │   ├── engine.rs          # llama.cpp wrapper, load/unload models
│   │   │   ├── session.rs         # Chat session management, KV cache
│   │   │   ├── streaming.rs       # Token streaming to frontend
│   │   │   └── gpu.rs             # GPU detection (CUDA/Vulkan)
│   │   │
│   │   ├── models/                # Model management
│   │   │   ├── mod.rs
│   │   │   ├── registry.rs        # Local model registry (installed models)
│   │   │   ├── downloader.rs      # HuggingFace download + progress
│   │   │   ├── hub.rs             # Browse/search HuggingFace models
│   │   │   └── quantization.rs    # Model info: quant level, size, etc.
│   │   │
│   │   ├── rag/                   # RAG pipeline
│   │   │   ├── mod.rs
│   │   │   ├── indexer.rs         # Document chunking + embedding
│   │   │   ├── retriever.rs       # Semantic search via LanceDB
│   │   │   ├── embeddings.rs      # Run embedding model
│   │   │   ├── chunker.rs         # Token-aware text splitting
│   │   │   └── parsers/           # Document parsers
│   │   │       ├── mod.rs
│   │   │       ├── pdf.rs         # PDF extraction (lopdf)
│   │   │       ├── docx.rs        # DOCX extraction (docx-rs)
│   │   │       ├── markdown.rs    # Markdown passthrough
│   │   │       └── text.rs        # Plain text
│   │   │
│   │   ├── agents/                # Agent / tool-use system
│   │   │   ├── mod.rs
│   │   │   ├── executor.rs        # Tool execution runtime
│   │   │   ├── planner.rs         # Multi-step agent planner
│   │   │   ├── tools/             # Built-in tools
│   │   │   │   ├── mod.rs
│   │   │   │   ├── web_search.rs  # Local web search tool
│   │   │   │   ├── calculator.rs  # Math evaluation
│   │   │   │   ├── file_ops.rs    # Read/write local files
│   │   │   │   ├── shell.rs       # Execute shell commands (sandboxed)
│   │   │   │   └── code_exec.rs   # Code interpreter (sandboxed)
│   │   │   └── mcp/               # MCP protocol support
│   │   │       ├── mod.rs
│   │   │       ├── server.rs      # MCP server implementation
│   │   │       ├── client.rs      # Connect to external MCP servers
│   │   │       └── protocol.rs    # MCP message types
│   │   │
│   │   ├── workflows/             # Multi-model orchestration
│   │   │   ├── mod.rs
│   │   │   ├── router.rs          # Route queries to best model
│   │   │   ├── chain.rs           # Sequential model chaining
│   │   │   ├── ensemble.rs        # Parallel inference + voting
│   │   │   └── pipeline.rs        # User-defined workflow graphs
│   │   │
│   │   ├── api/                   # OpenAI-compatible HTTP API
│   │   │   ├── mod.rs
│   │   │   ├── server.rs          # axum HTTP server
│   │   │   ├── routes/
│   │   │   │   ├── chat.rs        # POST /v1/chat/completions
│   │   │   │   ├── completions.rs # POST /v1/completions
│   │   │   │   ├── embeddings.rs  # POST /v1/embeddings
│   │   │   │   ├── models.rs      # GET /v1/models
│   │   │   │   └── health.rs      # GET /health
│   │   │   └── middleware.rs      # API key auth, rate limiting, CORS
│   │   │
│   │   ├── db/                    # Database layer
│   │   │   ├── mod.rs
│   │   │   ├── sqlite.rs          # Conversation history, settings
│   │   │   └── migrations.rs      # Schema migrations
│   │   │
│   │   ├── updater/               # Auto-update system
│   │   │   ├── mod.rs
│   │   │   └── checker.rs         # Check GitHub releases, notify
│   │   │
│   │   ├── notifications.rs       # System tray + toast notifications
│   │   └── commands.rs            # Tauri IPC command handlers
│   │
│   └── icons/                     # App icons
│
├── src/                           # React frontend
│   ├── main.tsx                   # Entry point
│   ├── App.tsx                    # Root component + routing
│   ├── components/
│   │   ├── chat/
│   │   │   ├── ChatView.tsx       # Main chat interface
│   │   │   ├── MessageBubble.tsx  # Message rendering (markdown/code)
│   │   │   ├── InputBar.tsx       # Chat input with file attach
│   │   │   ├── StreamingText.tsx  # Token-by-token rendering
│   │   │   └── ToolCallCard.tsx   # Show tool invocations inline
│   │   ├── models/
│   │   │   ├── ModelHub.tsx       # Browse & download models
│   │   │   ├── ModelCard.tsx      # Model info card
│   │   │   ├── DownloadProgress.tsx
│   │   │   └── ModelSelector.tsx  # Dropdown model picker
│   │   ├── rag/
│   │   │   ├── KnowledgeBase.tsx  # Manage document collections
│   │   │   ├── DocumentList.tsx   # View indexed documents
│   │   │   └── UploadZone.tsx     # Drag-drop file upload
│   │   ├── agents/
│   │   │   ├── AgentBuilder.tsx   # Visual agent/tool configuration
│   │   │   ├── ToolRegistry.tsx   # Available tools list
│   │   │   └── WorkflowEditor.tsx # Visual workflow builder
│   │   ├── workflows/
│   │   │   ├── WorkflowCanvas.tsx # Drag-drop pipeline builder
│   │   │   └── NodeEditor.tsx     # Configure workflow nodes
│   │   ├── settings/
│   │   │   ├── Settings.tsx       # Main settings page
│   │   │   ├── GPUSettings.tsx    # GPU config
│   │   │   ├── APISettings.tsx    # API server config
│   │   │   └── UpdateSettings.tsx # Auto-update preferences
│   │   └── layout/
│   │       ├── Sidebar.tsx        # Navigation sidebar
│   │       ├── TitleBar.tsx       # Custom window title bar
│   │       └── SystemTray.tsx     # Tray icon management
│   ├── stores/
│   │   ├── chatStore.ts           # Zustand: conversations
│   │   ├── modelStore.ts          # Zustand: models
│   │   ├── settingsStore.ts       # Zustand: settings
│   │   └── notificationStore.ts   # Zustand: notifications
│   ├── hooks/
│   │   ├── useInference.ts        # Hook for LLM inference
│   │   ├── useModels.ts           # Hook for model management
│   │   ├── useRAG.ts              # Hook for RAG operations
│   │   └── useStreaming.ts        # Hook for SSE/streaming
│   ├── lib/
│   │   ├── tauri.ts               # Tauri invoke wrappers
│   │   └── api.ts                 # API client helpers
│   └── styles/
│       └── globals.css            # Tailwind + custom styles
│
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
├── .github/
│   └── workflows/
│       ├── build.yml              # CI: build + test
│       └── release.yml            # CD: build installers + publish
├── PLAN.md                        # This file
└── README.md
```

---

## 4. IMPLEMENTATION PHASES

### Phase 1: Foundation (scaffold + inference engine)
1. Initialize Tauri 2 + React + Vite project
2. Set up Rust workspace with dependencies
3. Integrate llama.cpp via `llama-cpp-2` crate
4. Build model loading/unloading with GPU auto-detection
5. Implement token streaming over Tauri IPC events
6. Build basic chat UI with markdown rendering
7. SQLite database for conversation persistence
8. System tray with minimize-to-tray

### Phase 2: Model Management
9. HuggingFace API integration for model search/browse
10. Download manager with progress tracking + resume
11. Local model registry (scan models dir, read metadata)
12. Model Hub UI: search, filter by size/quant, 1-click download
13. Model selector in chat with quick-switch

### Phase 3: RAG Pipeline
14. Document parsers (PDF, DOCX, MD, TXT)
15. Token-aware chunking (512 tokens, 20% overlap)
16. Embedding generation via llama.cpp (nomic-embed-text)
17. LanceDB integration for vector storage
18. Semantic retrieval with citation tracking
19. Knowledge Base UI: drag-drop upload, collection management
20. RAG-augmented chat with source citations

### Phase 4: Agent & Tool System
21. Tool calling protocol (function calling via llama.cpp)
22. Built-in tools: calculator, file ops, code execution
23. Sandboxed tool execution runtime
24. MCP server implementation (expose tools via MCP)
25. MCP client (connect to external MCP servers)
26. Agent planner for multi-step reasoning
27. Agent UI: tool call visualization, step-by-step display

### Phase 5: Multi-Model Workflows
28. Model router (classify query → pick model)
29. Sequential chaining (model A output → model B input)
30. Parallel ensemble with result voting
31. User-defined workflow pipelines (JSON/visual)
32. Workflow canvas UI with drag-drop node editor

### Phase 6: Developer API
33. axum HTTP server running alongside Tauri
34. OpenAI-compatible endpoints: /v1/chat/completions, /v1/embeddings, /v1/models
35. Streaming SSE support
36. API key authentication
37. Request batching for throughput
38. API settings UI + docs page

### Phase 7: Updates & Notifications
39. Tauri updater plugin (check GitHub releases)
40. In-app update notifications with changelog
41. Background download + install
42. System notifications for: download complete, inference done, update available
43. Notification preferences in settings

### Phase 8: Polish & Release
44. Custom window title bar (frameless)
45. Dark/light theme
46. Keyboard shortcuts
47. Performance profiling + optimization
48. Windows installer (NSIS) via GitHub Actions
49. Auto-update endpoint on GitHub Releases
50. Landing page + documentation

---

## 5. KEY ARCHITECTURE DECISIONS

### 5a. llama.cpp Integration Strategy
**Approach**: Embed llama.cpp directly via `llama-cpp-2` Rust crate (NOT as a sidecar process).
- **Why**: Single binary distribution, no process management, lower latency, tighter control
- **GPU**: Auto-detect CUDA/Vulkan via `wgpu` or system queries; compile llama.cpp with appropriate backend
- **Memory**: Implement model unloading when switching; only 1 model loaded at a time (or 2 for workflows)

### 5b. Streaming Architecture
```
llama.cpp token callback
    → Rust channel (tokio::mpsc)
        → Tauri event emit("chat:token", payload)
            → React event listener → append to UI
```
- Each token emitted as it's generated (~10-50ms per token)
- Frontend accumulates tokens, renders incrementally with markdown

### 5c. RAG Data Flow
```
Upload → Parse → Chunk → Embed → Store (LanceDB)
                                      ↓
Query → Embed query → Search LanceDB → Top-K chunks
                                      ↓
                            Inject into prompt → LLM → Response with citations
```

### 5d. Tool Calling Protocol
```json
// Tool definition sent to LLM
{
  "type": "function",
  "function": {
    "name": "search_knowledge_base",
    "description": "Search indexed documents for relevant information",
    "parameters": {
      "type": "object",
      "properties": {
        "query": { "type": "string" }
      },
      "required": ["query"]
    }
  }
}
```
- LLM returns structured JSON tool call
- Rust executor parses, validates, runs tool in sandbox
- Result fed back to LLM for final response

### 5e. Auto-Update Flow
```
App start → Check GitHub Releases API → Compare semver
    → If newer: show notification banner
        → User clicks "Update" → Download .msi in background
            → Prompt restart → Install → Launch new version
```
Uses `tauri-plugin-updater` with GitHub Releases as the update server.

---

## 6. KEY RUST DEPENDENCIES (Cargo.toml)

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon", "protocol-asset"] }
tauri-plugin-updater = "2"
tauri-plugin-notification = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"
tauri-plugin-shell = "2"

# LLM Inference
llama-cpp-2 = "0.1"                # llama.cpp Rust bindings

# Vector DB & Embeddings
lancedb = "0.15"                   # Embedded vector store
arrow = "53"                       # Arrow format (LanceDB dep)

# Database
rusqlite = { version = "0.32", features = ["bundled"] }

# HTTP API Server
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors"] }

# Async Runtime
tokio = { version = "1", features = ["full"] }

# Document Parsing
lopdf = "0.34"                     # PDF
docx-rs = "0.4"                    # DOCX

# Downloads & HTTP
reqwest = { version = "0.12", features = ["stream", "json"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Utils
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
thiserror = "2"
```

---

## 7. KEY NPM DEPENDENCIES (package.json)

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-updater": "^2",
    "@tauri-apps/plugin-notification": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "react": "^19",
    "react-dom": "^19",
    "react-router-dom": "^7",
    "zustand": "^5",
    "react-markdown": "^10",
    "remark-gfm": "^4",
    "rehype-highlight": "^7",
    "shiki": "^3",
    "@radix-ui/react-dialog": "^1",
    "@radix-ui/react-dropdown-menu": "^2",
    "@radix-ui/react-tooltip": "^1",
    "tailwindcss": "^4",
    "lucide-react": "^0.470",
    "sonner": "^2"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "typescript": "^5.7",
    "vite": "^6",
    "@vitejs/plugin-react": "^4"
  }
}
```

---

## 8. BUILD & CI/CD

### GitHub Actions Release Pipeline
- Trigger on git tag `v*`
- Build Windows `.msi` installer via `tauri build`
- Upload to GitHub Releases
- Tauri updater checks releases for updates
- Code signing with Windows certificate (optional, recommended)

### Installer Features
- NSIS installer with custom install path
- Desktop shortcut + Start menu entry
- System tray auto-start option
- Bundled Visual C++ runtime

---

## 9. WHAT WE BUILD FIRST (Phase 1 Deliverables)

The initial implementation will scaffold the entire project and deliver a working chat with token streaming:

1. `cargo tauri init` with React + Vite template
2. Full project structure (all directories, module stubs)
3. llama.cpp integration with model loading
4. Token streaming from Rust → React
5. Basic chat UI with markdown rendering
6. SQLite for conversation history
7. System tray with minimize-to-tray
8. Model file picker (load local GGUF files)
9. GPU auto-detection
10. Tauri updater plugin configuration

This gives a **functional chat app** that can load and run any GGUF model locally with streaming — the foundation everything else builds on.
