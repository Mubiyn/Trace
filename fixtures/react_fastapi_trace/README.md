# react_fastapi_trace

Phase 2 fixture — L4 cross-layer trace.

| Layer | Expected |
|-------|----------|
| L3 UI | `button.onClick` **TRIGGERS** `placeCall` |
| L2 | `placeCall` **CALLS** `fetch` |
| L4 | `placeCall` **FETCHES** `POST /api/calls` |
| L3 API | `POST /api/calls` **HANDLES** `create_call` |

Languages: TypeScript (frontend), Python (backend).
