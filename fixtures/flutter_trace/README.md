# flutter_trace

Sprint 9 cross-layer fixture — Flutter `onPressed` → Dart handler → FastAPI route.

| Layer | Node |
|-------|------|
| L3 | `ElevatedButton.onPressed` |
| L2 | `placeCall` |
| L3/L4 | `POST /api/calls` → `create_call` |
