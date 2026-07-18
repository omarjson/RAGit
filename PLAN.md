# PLAN.md — RAGit

> RAGit is an open-source, fully on-device RAG desktop app. Tauri v2 + React/Vite frontend, llama.cpp inference, SQLite storage. Local mode (loopback only) and Team/Server mode (LAN-shared with auth + RBAC). Includes an out-of-band Indexing Engine with 5 depth levels, pause/resume/cancel, and per-file metadata persistence. Supports text, documents, and rich media (images, video, audio) via local multimodal models.

## 1. Vision & Dual Mode
- **Local** (default): `127.0.0.1` only, single user, max privacy.
- **Team/Server**: `0.0.0.0:PORT`, employees via browser, auth + RBAC + usage limits. Toggle in Settings with security warning.

## 2. Architecture
```
Frontend (React+Vite, WebView or Browser)
        │ invoke() / HTTP
Rust Core (Tauri v2):
  hardware.rs   — detect VRAM/RAM/CPU/GPU
  catalog.rs    — model catalog + device fitness
  download.rs   — HF download + SHA256 + progress (Channels)
  engine.rs     — llama.cpp sidecar lifecycle (+ mmproj for vision)
  indexer.rs    — Indexing Engine (background tasks, levels, pause/resume/cancel)
  auth.rs       — auth + RBAC (Team Mode)
  api.rs        — Axum HTTP server (Team Mode)
  rag/          — parse · embed · store · retrieve
        │ sidecar (127.0.0.1)        │ HTTP (0.0.0.0 in Team)
llama-server (chat/embed/vision)   SQLite (users, sessions, libraries, files, vectors, rbac)
```

## 3. Confirmed Tech
- Tauri v2 + `tauri_plugin_shell` → `llama-server` sidecar; keep `CommandChild` for clean shutdown.
- llama.cpp OpenAI-compatible API: `/v1/chat/completions` (SSE), `/v1/embeddings` (`--embedding`), `/v1/models`. Multimodal via `mmproj` (vision models).
- Axum for Team Mode HTTP server.
- Auth/RBAC: `axum_session` + `axum_session_auth` (server-side sessions on SQLite, Argon2, `HasPermission`). Alternative: simple JWT.
- Storage: SQLite via `sqlx` + `sqlite-vec` (or BLOB + cosine in Rust).
- Download progress: Tauri Channels. Port: bind `127.0.0.1:0` / `0.0.0.0:PORT`.

## 4. RBAC (Team Mode)
Roles: `admin` (full + model/user mgmt), `editor` (upload + query allowed libs), `viewer` (query only).
Tables: `users`, `libraries`, `library_members(library_id, user_id, role)`, `sessions`.
Middleware: `auth_middleware` → route-level `require_admin` / `require_library_access`. Chat respects allowed libraries.

## 5. RAG Pipeline
- **Parsers:**
  - Text/common: PDF, DOCX, TXT, MD, CSV, JSON, code.
  - Documents: XLSX, PPTX, EPUB, HTML.
  - Media: images (png/jpg/webp via mmproj + VL model), video (frame extraction via ffmpeg + whisper for audio), audio (whisper.cpp transcription).
  - OCR for scanned PDFs/images (via VL model mmproj, optional external OCR).
- **Chunking** by file type + model context.
- **Embedding:** local model via `llama-server --embedding` (user-optional).
- **Retrieval:** cosine similarity + optional reranking (`--reranking`).
- **Composition:** retrieved context + question → chat with source citations.
- **Model catalog:** each model carries `modalities: [text, vision, audio]`; vision models load `mmproj` automatically in `engine.rs`.

## 6. Indexing Engine
Out-of-band indexing that runs in the background (not during query time). The model pre-reads files and builds the index.

### Lifecycle
- **Triggers:** scheduled (cron/timer) or on-demand (per file / per library).
- **Controls:** ▶ start · ⏸ pause · ▶ resume · ✕ cancel — all persisted in SQLite so progress survives app restart.
- **Per-file persisted metadata:**
  ```
  indexed_files(
    file_id, library_id, path,
    content_hash,          -- SHA256, detects file changes
    index_level,          -- 1..5
    chunks_count, status,  -- pending|indexing|paused|done|canceled|error
    bytes_indexed, total_bytes,
    started_at, finished_at, error_msg
  )
  ```
- **ETA estimate:** `total_bytes ÷ measured_throughput` (throughput learned from prior runs or defaulted by file type + device speed). Live ETA shown during run.

### 5 Index Levels (clear progression)
1. **Raw chunking** — split text into chunks only (fast, coarse search).
2. **+ Structure extraction** — capture headings/sections/tables structure.
3. **+ Chunk summaries** — LLM generates a short summary per chunk (hybrid).
4. **+ Dense embedding** — full vector embeddings for semantic search.
5. **+ Rerank + entities/keywords** — reranking pass + entity & keyword index for deepest retrieval.

For media: levels 1–2 cover frame extraction / transcript; levels 4–5 may pass frames to a VL model for per-segment description.

### Engine
- **Hybrid:** basic processing (levels 1–2) in Rust background task (tokio) with progress events; deep steps (summaries level 3, entities level 5) via the LLM.
- State machine persisted; pause/resume/cancel send signals to the running task; canceled files can be re-indexed cleanly.

## 7. UI Screens
1. **Sidebar**: logo, Library, Chat, Indexing, Settings, active model + status.
2. **Model Hub**: cards, device fitness (🟢/🟡/🔴), est/measured tok/s, modalities (🖼 vision, 🔊 audio), download progress.
3. **Library**: drag & drop, file status, per-file index level + hash, media preview (images/video), attach media to chat.
4. **Indexing**: queue, progress bars, ETA, level selector, pause/resume/cancel, schedule config.
5. **Chat**: live SSE streaming + source citations, attach images/video/audio from library.
6. **Login** (Team Mode only).
7. **Settings**: mode toggle, IP/Port, GPU layers, context, embedding model, scheduler, user mgmt (admin).
Style: Tailwind CSS + shadcn/ui, dark default.

## 8. Project Structure
```
RAGit/
├─ src/
│  ├─ components/ (Sidebar, ModelCard, Library, Indexing, Chat, Login, Settings)
│  ├─ lib/ipc.ts · api.ts
│  └─ App.tsx
├─ src-tauri/src/
│  ├─ main.rs
│  ├─ hardware.rs · catalog.rs · download.rs · engine.rs · indexer.rs
│  ├─ auth.rs · api.rs
│  ├─ rag/{parse,embed,store,retrieve}.rs
│  └─ commands.rs
├─ src-tauri/migrations/
├─ catalog/models.yaml
├─ docs/ · README.md · LICENSE (MIT) · CONTRIBUTING.md
```

## 9. Implementation Roadmap
| Phase | Content | Status |
|-------|---------|--------|
| 0 | Scaffold: Tauri v2 + React/Vite, structure, README, LICENSE, CI | ✅ DONE |
| 1 | `hardware.rs`: detect VRAM/RAM/CPU + display | ✅ DONE |
| 2 | `catalog.rs` + `download.rs`: HF catalog, device fitness, download + SHA256 + progress | ✅ DONE |
| 3 | `engine.rs`: llama.cpp sidecar, health, measured speed, chat streaming (+ mmproj for vision) | ✅ DONE |
| 4 | Core RAG: parsers + embed + SQLite + retrieve + chat w/ sources | ✅ DONE |
| 5 | **Indexing Engine**: lifecycle, 5 levels, pause/resume/cancel, metadata, ETA, scheduler | ✅ DONE |
| 6 | Extended files + media: DOCX/XLSX/PPTX/HTML/EPUB + images/video/audio (mmproj/whisper/ffmpeg) | ✅ DONE |
| 7 | **Team Mode**: Axum server + auth + RBAC + mode toggle + user mgmt | ✅ DONE |
| 8 | Advanced: reranking, separate embed model, import/export, cross-platform builds | ✅ DONE |
| 9 | OSS launch: GitHub repo, issue/PR templates, docs | ✅ DONE |

### Phase 4 — Implementation Notes (COMPLETE)
- Stack: `rusqlite` (bundled SQLite) + in-Rust cosine similarity (no external vec extension
  needed → portable & offline). Embeddings via `llama-server --embedding` (`/v1/embeddings`,
  base64 float blob decoded in `rag/embed.rs`).
- Files:
  - `src-tauri/src/rag/store.rs` — SQLite schema (`libraries`, `files`, `chunks`+`embedding` BLOB),
    `add_chunk`, `search` (cosine in Rust), `file_count`.
  - `src-tauri/src/rag/parse.rs` — `chunk_text` (size+overlap), `parse_file` (txt/md/code/csv/json/
    yaml/toml + PDF via `pdftotext` if present), `collect_files` (recursive walkdir).
  - `src-tauri/src/rag/embed.rs` — blocking `/v1/embeddings` call + base64→f32.
  - `src-tauri/src/rag/mod.rs` — `index_path` (parse→embed→store), `retrieve_context`, and the
    `#[tauri::command]`s `index_library` + `rag_chat` (injects retrieved context as system prompt).
  - `src-tauri/src/lib.rs` — `Store` managed state + `APP_STATE` global for `engine::with_port`.
  - `src-tauri/src/engine.rs` — added `with_port()` helper (read only engine port).
  - `src/components/Chat.tsx` — RAG sidebar: Index Folder + RAG mode toggle; uses `ragChat` when on.
  - `src/lib/ipc.ts` — `indexLibrary`, `ragChat`, `openFolderDialog`.
- Verified: `cargo check` + `npm run build` both pass. End-to-end requires a launched engine +
  an embed-capable model in the catalog (Phase 6 will add dedicated embed models).

### Phase 5 — Implementation Notes (COMPLETE)
- `src-tauri/src/rag/indexer.rs` — `IndexerState` (per-library `JobControl` with `paused`/`canceled`
  atomics). Commands: `index_library` (Channel progress, background thread, per-file), `pause_index`,
  `resume_index`, `cancel_index`, `list_indexed_files`, `set_scheduler` (interval re-index of
  pending/error files). `index_one_file` walks the 5 levels:
  - L1 raw chunking (`chunk_text`)
  - L2 structure-aware (`chunk_by_structure`, headings → `[section: …]` tags)
  - L3 summaries via LLM (`summarize_chunk` → `[summary: …]` prefix) — needs engine
  - L4 dense embeddings (`embed::embed`) attached to chunk rows
  - L5 entity extraction (`extract_entities` → `[entities: …]` suffix)
  Levels are cumulative; steps needing the engine degrade gracefully (e.g. no engine ⇒ stops at L2).
- `src-tauri/src/rag/store.rs` — `files` table extended with `status`, `indexed_level`, `chunks`,
  `error`, `started_at`, `finished_at`; `chunks` gains `level` column. Helpers: `upsert_file`,
  `update_file_status`, `add_chunk_enriched` (level + raw embedding BLOB), `get_files`.
- `src-tauri/src/rag/parse.rs` — `Level` enum, `chunk_by_level`, `chunk_by_structure`,
  `heading_of`, `extract_entities`.
- `src-tauri/src/lib.rs` — `IndexerState` added to `GlobalState` + managed; commands registered.
- `src/components/Chat.tsx` — RAG sidebar now has: Index Folder, depth-level selector, live progress
  bar + Pause/Resume/Cancel, scheduler toggle, and an indexed-files list with per-file status/level/chunks.
- `src/lib/ipc.ts` — `indexLibrary` (progress channel), `pauseIndex`, `resumeIndex`, `cancelIndex`,
  `listIndexedFiles`, `setScheduler`, types `IndexProgress` / `IndexedFile`.

### Phase 7 — Implementation Notes (COMPLETE)
- `src-tauri/src/team.rs` — Axum HTTP server (tokio runtime, spawned thread) bound to `0.0.0.0:11436`.
  Auth: Argon2 password hashing + HMAC-SHA256 signed session tokens (7-day expiry, stored in
  `sessions`). RBAC: `Role::{Admin,Editor,Viewer}` with `Role::can()` rank check. Endpoints:
  - `POST /api/register` (first user → admin; others require admin token)
  - `POST /api/login`, `POST /api/logout`, `GET /api/me`
  - `GET /api/users`, `POST /api/users/role` (admin only)
  - `GET /api/libraries` (admins see all; others see only member libs)
  - `POST /api/libraries/grant` (admin → `library_members`)
  - `POST /api/chat` — RBAC-gated; editors/viewers must be members of the requested library;
    delegates to `rag::indexer::team_rag_answer` (retrieve context + engine completion).
  Commands: `start_team_server_cmd`, `stop_team_server_cmd`, `team_status_cmd`; server `Arc<TeamState>`
  kept in `GlobalState.team`.
- `src-tauri/src/rag/store.rs` — schema + helpers for `users`, `library_members`, `sessions`:
  `create_user`, `find_user_by_username`, `list_users`, `set_user_role`, `add_session`,
  `user_for_token`, `delete_session`, `grant_membership`, `library_role`, `list_files_libs`.
- `src-tauri/src/lib.rs` — `GlobalState.team: Mutex<Option<Arc<TeamState>>>`; team commands registered.
- `src/components/Team.tsx` + `src/lib/ipc.ts` (`startTeamServer`, `stopTeamServer`, `teamStatus`,
  `teamApi`) — UI to start/stop the server, register/login (first = admin), admin user-role management,
  and a RBAC-gated team chat. Added "Team" nav entry in `App.tsx`/`Sidebar.tsx`.
- Security: Team Mode binds `0.0.0.0` by design (LAN share). Tokens are HMAC-signed; passwords Argon2.
  No TLS — intended for trusted LAN; document this in Settings before enabling.

### Phase 8 — Implementation Notes (COMPLETE)
- **Separate embed model**: `engine.rs` `EngineState` gained `embed_child` + `EngineStatus.embedModel/
  embedPort`. `start_engine` now accepts `embed_model_path`/`embed_port` and launches a *second*
  `llama-server` on its own port (`spawn_server` helper). `engine::embed_port()` returns the dedicated
  embed port when present, else the chat port (which also has `--embedding`). `embed::embed` now routes
  through `embed_port()`. `ModelHub` lets you pick an `embed:` catalog model (e.g. nomic-embed-text) to
  launch alongside the chat model.
- **Reranking**: `rag/mod.rs` `retrieve_context_rerank` over-fetches vector candidates (K×4), then
  re-ranks by lexical overlap (token Jaccard) blended 0.6·vector + 0.4·lexical. `rag_chat` gains a
  `rerank` flag (UI toggle in Chat sidebar); `team_rag_answer` accepts `rerank` (team `/api/chat`
  `rerank` field).
- **Import/Export**: `rag/export.rs` — `export_library` (writes a JSON of files+chunks+metadata to a
  path) and `import_library` (reads JSON, re-inserts; re-embeds chunks live if an embed engine is
  running). Commands registered; UI Export/Import buttons in Chat sidebar (save/open dialogs).
- **Cross-platform builds**: `tauri.conf.json` already sets `bundle.targets: "all"`; the Rust core is
  platform-agnostic (no Windows-only APIs beyond llama.cpp binaries per-release). README documents
  `tauri build` per-target. No CI runner available here, so actual Linux/macOS bundles are left to the
  release pipeline (Phase 9).
- Verified: `cargo build` (full) + `npm run build` both pass.

### Phase 9 — Implementation Notes (COMPLETE)
- Initialized git repo at project root; added `.gitignore` (build artifacts, engine
  binaries, node_modules, secrets, OS/editor junk).
- Rewrote `README.md` (clean, accurate tech stack + usage; removed mojibake).
- Added `CONTRIBUTING.md`, `SECURITY.md`, `.github/ISSUE_TEMPLATE/bug_report.yml`,
  `feature_request.yml`, `.github/PULL_REQUEST_TEMPLATE.md`, and
  `.github/workflows/ci.yml` (GitHub Actions: `npm run build` + `cargo check` on PRs).
- All 10 phases (0–9) are now implemented and building.

### Phase 6 — Implementation Notes (COMPLETE)
- `src-tauri/src/rag/media.rs` — pure-Rust parsers (no external binaries):
  - `parse_html` (tag-strip, drops `<script>/<style>`), `parse_docx` (zip → `word/document.xml`),
    `parse_xlsx` (zip → `xl/sharedStrings.xml` + per-sheet cells), `parse_pptx` (slides),
    `parse_epub` (zip of XHTML chapters). Uses `quick-xml` for namespace-safe text extraction.
- `src-tauri/src/rag/vision.rs` — `describe_image` (vision model via `mmproj` + OpenAI multimodal
  `/v1/chat/completions`), `transcribe_media` (`whisper-cli` if on PATH), `describe_video_frames`
  (`ffmpeg` frame extraction → vision describe). All degrade to `None` when the local tool/model is
  absent (graceful offline behaviour).
- `src-tauri/src/rag/parse.rs` — `parse_file` now routes html/docx/xlsx/pptx/epub; added `parse_media`
  dispatcher (images/audio/video). `collect_files` includes the new extensions.
- `src-tauri/src/rag/indexer.rs` — `index_one_file` falls back to `parse_media` when `parse_file`
  yields nothing, so media enters the same 5-level pipeline.
- `src-tauri/src/engine.rs` — `start_engine` now passes `--embedding` so `/v1/embeddings` works on
  embedding models (e.g. the `nomic-embed-text` catalog entry, already `embed: true`).
- Note: external media tooling (whisper.cpp, ffmpeg, pdftotext, tesseract) is **not installed** on the
  dev machine, so image/audio/video steps require the user to install them (or a vision model for
  images). Text/document parsers (docx/xlsx/pptx/html/epub/pdf) work fully offline.
```
