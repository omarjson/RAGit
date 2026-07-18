# Contributing to RAGit

Thanks for your interest in improving RAGit! This guide covers how to get set up and
what we expect from contributions.

## Development Setup

1. Fork and clone the repo.
2. Install prerequisites: Node 18+, Rust stable (MSVC on Windows).
3. `npm install`
4. `npm run tauri dev` to run the app.

## Project Conventions

- **Rust** (`src-tauri/`): `cargo fmt` and `cargo clippy` before pushing. Keep commands
  in their domain modules (`engine.rs`, `rag/`, `team.rs`). Document public functions.
- **TypeScript/React** (`src/`): run `npm run build` (runs `tsc`). Follow the existing
  Tailwind/shadcn style. New backend commands must be wired in `src-tauri/src/lib.rs`
  and exposed via `src/lib/ipc.ts`.
- Keep changes focused; one feature/bug per PR.

## Pull Requests

- Branch from `main` (e.g. `feat/rerank` or `fix/zip-extract`).
- Fill out the PR template; link the relevant issue.
- Ensure `npm run build` and `cargo check` pass.
- Add tests or manual verification steps where reasonable.

## Reporting Bugs

- Use the bug report template. Include OS, hardware, model used, and steps to reproduce.
- Attach logs if possible (redact any secrets).

## Code of Conduct

Be respectful and constructive. We follow the
[Contributor Covenant](https://www.contributor-covenant.org/) — keep discussions friendly
and inclusive.

## License

By contributing, you agree your contributions are licensed under the MIT License.
