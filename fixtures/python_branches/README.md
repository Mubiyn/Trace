# python_branches

Sprint 8 fixture — CFG decision branches.

| Branch | Outcomes |
|--------|----------|
| `if permitted` | `on_ok()` or `on_denied()` |

Expected edges: `handle` → `BRANCHES_TO` → branch → `BRANCHES_TO` → callees.
