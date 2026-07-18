# 🐙 RAGit

> On-device RAG desktop app. Pick a local model, see if your machine can run it,
> download it, and chat with your files — **fully offline, fully yours**.

RAGit is an open-source desktop application built with **Tauri v2 + React** that runs
LLMs locally via **llama.cpp**. It lets you:

- 🔎 **Choose a model** and instantly see whether your hardware can handle it
  (🟢 fits / 🟡 tight / 🔴 too big) plus estimated and measured tokens/sec.
- ⬇️ **Download** GGUF models from Hugging Face with live progress + SHA256 verification.
- 💬 **Chat** with an OpenAI-compatible local server (`llama-server`).
- 📚 **Build a library** of files (text, documents, images, video, audio) and ask
  questions over them via Retrieval-Augmented Generation.
- 🗂️ **Index out-of-band** with 5 depth levels, pause / resume / cancel, and persisted
  per-file metadata (hash, level, progress).
- 👥 **Team Mode** (optional): expose the app on your LAN with authentication and
  role-based access control (admin / editor / viewer).

## Dual Mode

| Mode | Bind | Use case |
|------|------|----------|
| **Local** (default) | `127.0.0.1` | Single user, max privacy |
| **Team/Server** | `0.0.0.0:PORT` | Company hub, employees via browser |

> ⚠️ Team Mode binds `0.0.0.0` by design (LAN share). Tokens are HMAC-signed and
> passwords are Argon2-hashed, but there is **no TLS** — only enable it on a trusted
> network.

## Tech Stack

- **Tauri v2** desktop shell (Rust)
- **React + Vite + Tailwind** frontend
- **llama.cpp** (`llama-server`) sidecar for chat / embeddings / vision
- **Axum** HTTP server for Team Mode
- **SQLite** (`rusqlite`, bundled) for users, sessions, libraries, files, vectors
- In-Rust cosine similarity for retrieval (no external vector extension required)

## Getting Started

### Prerequisites

- **Node.js 18+** and npm
- **Rust** (stable) with the **MSVC** toolchain on Windows
  (Linux/macOS: the platform C toolchain)
- (Optional, for media) `ffmpeg`, `whisper.cpp`, `pdftotext`, `tesseract`

### Build & Run

```bash
# install JS dependencies
npm install

# run in dev mode
npm run tauri dev

# build the installer (nsis / msi on Windows, etc.)
npm run tauri build
```

The first time you launch a model, RAGit downloads the `llama.cpp` `llama-server`
binary for your platform automatically.

## Using RAGit

1. Open **Models**, pick a quant that fits your RAM/VRAM, **Download** then **Launch**.
2. Open **Indexing**, choose a depth level (L1 raw → L5 rerank), click **Index Folder**.
   Watch progress; you can **Pause / Resume / Cancel** anytime.
3. Open **Chat**, enable **RAG mode** (and optionally **Rerank**), and ask questions —
   answers cite the source files.
4. (Optional) **Team** tab → **Start Team Server** to share the library over your LAN.
   The first account created becomes admin.

## Project Layout

```
src-tauri/src/
  hardware.rs   detect VRAM/RAM/CPU/GPU
  catalog.rs    model catalog + device fitness
  download.rs   HF download + SHA256 + progress
  engine.rs     llama.cpp sidecar lifecycle (+ embed engine)
  chat.rs       streaming chat completions
  team.rs       Axum server + auth + RBAC
  rag/          parse · embed · store · indexer · media · vision · export
catalog/models.yaml   curated model catalog
```

See [`PLAN.md`](./PLAN.md) for the full architecture and roadmap.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md). Issues and PRs welcome!

## License

[MIT](./LICENSE)
