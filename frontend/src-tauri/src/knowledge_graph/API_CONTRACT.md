# LightRAG API Contract (Week 1 target)

Source pin: `4e1f95269b32cd803cb96e0acc24d922aa6b6932`

Week 1 implementation must target the verified LightRAG HTTP contract below:

- `POST /documents/text` — insert document text
- `POST /query` — query the knowledge graph
- `GET /health` — server health/status
- `GET /documents/pipeline_status` — document pipeline status
- `GET /documents/track_status/{track_id}` — track a document job by ID

Auth, when configured, uses the `X-API-Key` request header. The default deployment may be open if no key is set.

## Version caveat

LightRAG v1.5.0rc2 and later changed file-processing, parser routing, multimodal analysis, JSON extraction, and storage behavior. Treat this contract as pinned to the commit above; do not assume older query sketches are valid.

## Week 1 implementation rule

Use `POST /query` for all query flows. Do not add executable Rust logic in this note.
