# graph-engine

Query layer over indexed code graphs.

## API

```rust
use graph_engine::GraphEngine;
use graph_indexer::fixture_path;

let engine = GraphEngine::index(&fixture_path("python_simple"))?;

let hits = engine.search("greet", 10)?;
let callees = engine.callees("sym:main.py:function:greet:4")?;
let callers = engine.callers("sym:utils.py:function:format_message:1")?;
let impact = engine.impact("sym:utils.py:function:format_message:1", 2)?;
let trace = engine.trace("sym:main.py:function:greet:4", 5)?;
let roots = engine.entry_points()?;
let slice = engine.subgraph("utils.py", true)?;
```

## Queries

| Method | Description |
|--------|-------------|
| `search(q, limit)` | Fuzzy symbol search by name/path/id |
| `callers(id)` | Incoming `CALLS` edges |
| `callees(id)` | Outgoing `CALLS` edges |
| `impact(id, depth)` | Reverse `CALLS` BFS blast radius |
| `trace(id, depth)` | Forward call chain with sibling branches |
| `entry_points()` | Symbols with no incoming `CALLS` |
| `subgraph(scope, boundary)` | Scoped nodes + ingress/egress ghosts |

## Tests

```bash
cargo test -p graph-engine
```
