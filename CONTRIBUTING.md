# Contributing to Trace

Thank you for your interest in contributing. Trace is a Rust indexing engine with TypeScript surfaces (web, VS Code, desktop). This guide covers local setup, tests, and how to submit changes.

## Prerequisites

| Tool | Version | Used for |
|------|---------|----------|
| [Rust](https://rustup.rs/) | 1.75+ | `graph-indexer`, `graph-engine`, `graph-server` |
| [Node.js](https://nodejs.org/) | 20+ | Web UI, VS Code extension, desktop shell |
| [git](https://git-scm.com/) | any recent | Clone GitHub repos for indexing |

Optional: `clippy` (`rustup component add clippy`) for lint checks matching CI.

## Getting started

```bash
git clone https://github.com/Mubiyn/Trace.git
cd Trace

# Rust workspace
cargo build --workspace
cargo test --workspace

# JavaScript workspace (from repo root)
npm install
npm test
npm run build
```

## Running locally

**Terminal 1 — API server (port 9847):**

```bash
cargo run -p graph-server --bin graph-server
```

**Terminal 2 — Web UI (port 5173):**

```bash
npm run dev:web
```

Open http://localhost:5173, enter a repository path or GitHub URL, and click **Index**.

### Index a public GitHub repo

```bash
curl -X POST http://127.0.0.1:9847/index \
  -H 'Content-Type: application/json' \
  -d '{
    "path": ".",
    "gitUrl": "https://github.com/encode/starlette",
    "gitRef": "main",
    "persist": true
  }'
```

Poll `GET /index/{job_id}` until `status` is `complete`, then query via the UI or `POST /query`.

## Project layout

```
├── crates/
│   ├── graph-indexer/   # tree-sitter parsing, framework plugins, SQLite
│   ├── graph-engine/    # trace, impact, subgraph queries
│   └── graph-server/    # HTTP API, MCP, hosted multi-repo registry
├── packages/graph-ui/   # shared React + Cytoscape graph viewer
├── apps/
│   ├── web/             # Vite dev app
│   ├── vscode-extension/
│   └── desktop/         # Tauri shell
├── fixtures/            # golden-test sample repos
└── api/openapi.yaml     # HTTP contract
```

## Making changes

### Rust (engine / indexer)

1. Add or update tests under `crates/*/tests/` or `#[cfg(test)]` modules.
2. Use fixtures in `fixtures/` for integration tests — see `crates/graph-indexer/tests/fixtures.rs`.
3. Run `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` before opening a PR.

**Adding a language grammar:** follow the notes in `crates/graph-indexer/README.md`.

**Adding a framework plugin** (routes, UI handlers): add a module under `crates/graph-indexer/src/framework/` and register it in `framework/mod.rs`.

### TypeScript (UI / apps)

1. Shared UI logic lives in `packages/graph-ui/`.
2. Run `npm test -w @graph/ui` while iterating.
3. Match existing patterns in `GraphApp.tsx`, `modes.ts`, and `navigation.ts`.

### API changes

If you change HTTP routes or request shapes, update `api/openapi.yaml` and integration tests in `crates/graph-server/tests/`.

## Pull request checklist

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes (or explain why not)
- [ ] `npm test` and `npm run build` pass for UI changes
- [ ] New behavior has tests (fixture golden, API contract, or UI unit test)
- [ ] Commit messages describe **why**, not just what

## Code style

- **Rust:** follow existing crate conventions; prefer focused diffs over large refactors.
- **TypeScript:** match surrounding React patterns; no unnecessary abstractions.
- **Comments:** only where behavior is non-obvious.

## Reporting issues

Include:

- OS and Rust/Node versions
- Steps to reproduce (repo path or GitHub URL, scope, view mode)
- Expected vs actual graph behavior
- Server logs if relevant (`RUST_LOG=debug cargo run …`)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](./LICENSE).
