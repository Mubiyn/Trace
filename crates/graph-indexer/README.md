# graph-indexer

Language-agnostic repository indexer for [Graph](../../README.md).

## Tiers

| Tier | What gets indexed |
|------|-------------------|
| **L0** | File tree (any path) |
| **L1** | + tree-sitter symbols (functions, classes, imports) |
| **L2** | + resolved `CALLS` and `IMPORTS` edges with confidence |
| **L3** | + `ui_element` / `route` nodes; `TRIGGERS` / `HANDLES` edges |
| **L4** | + `FETCHES` cross-layer edges (e.g. TS `fetch` → Python route) |

## Framework plugins (L3/L4)

| Framework | Module | Edges |
|-----------|--------|-------|
| React/JSX `onClick` | `src/framework/react.rs` | `TRIGGERS` |
| FastAPI routes | `src/framework/fastapi.rs` | `HANDLES` |
| Express routes | `src/framework/express.rs` | `HANDLES` |
| Spring `@GetMapping` | `src/framework/spring.rs` | `HANDLES` |
| Gin `router.GET` | `src/framework/gin.rs` | `HANDLES` |
| Flask `@app.route` | `src/framework/flask.rs` | `HANDLES` |
| Django `path(...)` | `src/framework/django.rs` | `HANDLES` |
| Actix `#[get(...)]` | `src/framework/actix.rs` | `HANDLES` |
| Flutter `onTap` | `src/framework/flutter.rs` | `TRIGGERS` |
| `fetch('/api/...')` | `src/framework/react.rs` | `FETCHES` |
| Dart `http.post` | `src/framework/flutter.rs` | `FETCHES` |

Add a new framework by extending `src/framework/` and registering in `framework/mod.rs`.

## Bundled grammars (L1)

| Language | Crate |
|----------|-------|
| Python | `tree-sitter-python` |
| Rust | `tree-sitter-rust` |
| Go | `tree-sitter-go` |
| JavaScript | `tree-sitter-javascript` |
| TypeScript | `tree-sitter-typescript` |
| Ruby | `tree-sitter-ruby` |
| Java | `tree-sitter-java` |
| PHP | `tree-sitter-php` |
| Swift | `tree-sitter-swift` |
| Dart | regex L1 (`extract/dart.rs`) |

## Adding a language

1. Add `tree-sitter-<lang>` to `Cargo.toml`.
2. Add a `LangQuery` entry in `src/extract/mod.rs` with a symbols query.
3. Ensure `detect_language()` in `src/language.rs` maps the file extension.
4. Add a fixture under `fixtures/` and a golden graph under `expected/`.
5. Run `cargo test -p graph-indexer`.

### Symbol query tips

- Capture the definition node as `@item` and the name as `@name`.
- `match_nodes()` picks identifier vs definition captures automatically.
- Python imports use a separate query in `src/extract/python.rs`.

## API

```rust
use graph_indexer::{index, export_graph_json};
use std::path::Path;

let result = index(Path::new("/path/to/repo"))?;
println!("files: {}, symbols: {}", result.file_count, result.symbol_count);

let json = export_graph_json(Path::new("/path/to/repo"))?;
```

## Export golden fixture

```bash
cargo run -p graph-indexer --example export_fixture -- python_simple \
  2>/dev/null > fixtures/python_simple/expected/graph.json
```

## Tests

```bash
cargo test -p graph-indexer
```
