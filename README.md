# Trace

**Map any codebase into navigable roots → branches → outcomes.**

Trace indexes a repository (local path or GitHub URL) and builds an interactive graph: UI controls and routes as **roots**, calls and control-flow decisions as **branches**, cross-layer paths (e.g. `fetch` → API handler) as **outcomes**. Explore in the **browser**, **VS Code / Cursor**, or a **desktop app** — or query the same graph from scripts and AI agents via **HTTP** and **MCP**.

## Features

- **Language-agnostic indexing** — file tree (L0), symbols via tree-sitter (L1), call/import graphs (L2)
- **Framework-aware layers** — React `onClick`, FastAPI/Flask/Django routes, Express, Flutter, Spring, Gin, Actix, and more
- **Control-flow branches** — `if` / loop decision nodes with `BranchesTo` edges
- **Cross-layer traces** — UI → handler → HTTP route → backend function
- **Drill-down navigation** — click nodes to focus; breadcrumbs and **Back** to return
- **Runtime overlay** — import OpenTelemetry JSON to highlight observed paths
- **Multi-repo hosted mode** — persist indexes under `~/.graph/hosted` (local MVP)
- **MCP tools** — `graph_search`, `graph_trace`, `graph_impact`, `graph_entry_points` for AI assistants

## Quick start

### Prerequisites

- Rust 1.75+
- Node.js 20+
- git (for cloning GitHub repos)

### 1. Start the server

```bash
cargo run -p graph-server --bin graph-server
```

API listens on **http://127.0.0.1:9847**.

### 2. Start the web UI

```bash
npm install
npm run dev:web
```

Open **http://localhost:5173**.

### 3. Index a repository

In the UI:

1. Enter a **local path** (e.g. `./fixtures/react_fastapi_trace`) or a **GitHub URL** (e.g. `https://github.com/encode/starlette`)
2. Optionally set **scope** (e.g. `frontend`) to narrow the subgraph
3. Click **Index**, then **Refresh**

Or via HTTP:

```bash
# Local path
curl -X POST http://127.0.0.1:9847/index \
  -H 'Content-Type: application/json' \
  -d '{"path": "/absolute/path/to/your/repo"}'

# GitHub (shallow clone)
curl -X POST http://127.0.0.1:9847/index \
  -H 'Content-Type: application/json' \
  -d '{
    "path": ".",
    "gitUrl": "https://github.com/encode/starlette",
    "gitRef": "main",
    "persist": true
  }'
```

Poll job status: `GET /index/{job_id}`

### 4. Query the graph

```bash
# Subgraph for a scope
curl -X POST http://127.0.0.1:9847/query \
  -H 'Content-Type: application/json' \
  -d '{"op": "subgraph", "scope": "frontend", "boundary": true}'

# Trace from a symbol
curl -X POST http://127.0.0.1:9847/query \
  -H 'Content-Type: application/json' \
  -d '{"op": "trace", "id": "sym:frontend/App.tsx:function:placeCall:1", "depth": 8}'
```

See [api/openapi.yaml](./api/openapi.yaml) for the full contract.

## View modes (web UI)

| Mode | Shows |
|------|--------|
| **Broad** | Directory / file tree (`Contains` edges) |
| **Isolation** | Symbols and semantic edges (`Calls`, `Triggers`, `Fetches`, …) |
| **Tree** | Top-down layout from entry roots |

Click a node to **drill in**; use **Back** or breadcrumbs to navigate. The **Trace** panel lists roots → branches for the selected node.

## Surfaces

| Surface | How to run |
|---------|------------|
| **Web** | `npm run dev:web` |
| **VS Code / Cursor** | Open `apps/vscode-extension`, run Extension Development Host |
| **Desktop** | `npm run dev -w @graph/desktop` (Tauri) |
| **MCP** | `cargo run -p graph-server --bin graph-mcp` |
| **HTTP API** | `cargo run -p graph-server --bin graph-server` |

All surfaces talk to the same `graph-server` on port **9847**.

## Environment variables

| Variable | Purpose |
|----------|---------|
| `GRAPH_HOSTED_DIR` | Directory for persisted multi-repo indexes (default: `~/.graph/hosted`) |
| `RUST_LOG` | Server log level (e.g. `graph_server=debug`) |

## Development

```bash
# Rust tests (engine + indexer + server)
cargo test --workspace

# UI + extension tests
npm install
npm test

# Production UI build
npm run build
```

See [CONTRIBUTING.md](./CONTRIBUTING.md) for layout, fixtures, and PR guidelines.

## Building surfaces

All UIs are **clients** of `graph-server` on `127.0.0.1:9847`. Build the server first:

```bash
cargo build --release -p graph-server
# binary: target/release/graph-server
```

### Web app

```bash
npm install
npm run build:web
# output: apps/web/dist/
npm run preview -w @graph/web   # local preview of production build
```

**GitHub Pages** (automated on push to `main` via [`.github/workflows/pages.yml`](./.github/workflows/pages.yml)):

1. In the repo on GitHub: **Settings → Pages → Build and deployment → GitHub Actions**
2. After deploy, the demo lives at **https://mubiyn.github.io/Trace/**
3. You still need **`graph-server` running locally** — the hosted page is static HTML/JS only

For a custom Pages path, set `VITE_BASE` when building (default in CI: `/Trace/`).

### Desktop app (Tauri)

```bash
npm install
npm run icon -w @graph/desktop      # once, regenerates src-tauri/icons/
npm run dev:desktop                 # dev mode
npm run build:desktop               # installers in apps/desktop/src-tauri/target/release/bundle/
```

The desktop shell tries to spawn `graph-server` from your `PATH` or `target/debug/graph-server`. For a release build, run the `graph-server` binary alongside the app.

**CI:** tag a release (`git tag v0.1.0 && git push origin v0.1.0`) — [`.github/workflows/release.yml`](./.github/workflows/release.yml) builds macOS / Windows / Linux bundles and uploads a draft GitHub Release.

### VS Code / Cursor extension

```bash
npm install
npm run build:extension             # compile extension + webview
npm run package:extension           # → apps/vscode-extension/trace-0.1.0.vsix
```

**Install locally:** Extensions panel → `…` → **Install from VSIX** → select the `.vsix` file.

**Run in dev:** open `apps/vscode-extension` in VS Code, press F5 (Extension Development Host), run command **Trace: Open Panel**.

**Publish to Marketplace** (optional):

1. Create a [publisher](https://marketplace.visualstudio.com/manage) (e.g. `mubiyn`)
2. `npx vsce login mubiyn`
3. `npm run package:extension && npx vsce publish -w trace`

Same `.vsix` works in **Cursor** (Install from VSIX). JetBrains support is not included yet.

### GitHub Releases

Push a version tag to build everything:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow produces a **draft** release with:

| Asset | Description |
|-------|-------------|
| `graph-server-*` | API server per OS/arch |
| `desktop-*` | Tauri bundles (`.dmg`, `.msi`, `.AppImage`, etc.) |
| `trace-*.vsix` | VS Code / Cursor extension |

## Example fixture

The `fixtures/react_fastapi_trace` repo demonstrates L3/L4 tracing:

```
PlaceCallButton → button.onClick → placeCall → POST /api/calls → create_call
```

```bash
curl -X POST http://127.0.0.1:9847/index \
  -H 'Content-Type: application/json' \
  -d '{"path": "./fixtures/react_fastapi_trace"}'
```

Set scope to `frontend` in the UI and use **Isolation** or **Tree** mode.

## Architecture

```
Web / IDE / Desktop / MCP
        │
        ▼
  graph-server (:9847)
        │
        ▼
  graph-engine (trace, impact, subgraph)
        │
        ▼
  graph-indexer (tree-sitter, SQLite)
```

## License

[MIT](./LICENSE) — Copyright (c) 2026 Mubiyn
