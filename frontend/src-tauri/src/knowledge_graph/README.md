# Knowledge Graph

Deterministic test data and local QA notes for the knowledge-graph module.

## Dev commands

Run from `frontend/src-tauri`:

```bash
cargo test knowledge_graph
cargo test knowledge_graph -- --nocapture
```

Common module filters:

- `cargo test config`
- `cargo test provider`
- `cargo test types`
- `cargo test test_fixtures`

## Compose lifecycle

Run from the repo root:

```bash
docker compose -f docker-compose.kg.yml --env-file kg.env.example config
docker compose -f docker-compose.kg.yml --env-file kg.env.example up -d
docker compose -f docker-compose.kg.yml ps
docker compose -f docker-compose.kg.yml logs -f
docker compose -f docker-compose.kg.yml down
docker compose -f docker-compose.kg.yml down -v
```

## Week 2 Capabilities

The knowledge-graph module in Week 2 supports the following features:

- **Profile management** — Add, edit, delete, and test LightRAG endpoint profiles via Tauri commands (`api_get_knowledge_graph_settings`, `api_save_knowledge_graph_settings`, `api_test_knowledge_graph_profile`). Profiles include embedding configuration (provider, model, dimensions) and health-check support.
- **Default profile selection** — Persist a default profile in settings. The default is always `none` until the user explicitly sets one.
- **Pre-recording opt-in** — The frontend can persist a KG profile selection intent for a meeting before recording starts (`api_set_meeting_knowledge_graph_selection`). This is advisory only and does **not** trigger any automatic behavior.
- **Explicit post-save ingestion** — After a meeting is saved, transcript chunks are ingested only when `api_ingest_meeting_to_knowledge_graph` is explicitly called. Returns a summary of submitted, already-submitted, and failed chunks.
- **Meeting status tracking** — `api_get_meeting_knowledge_graph_status` returns chunk counts (submitted, failed, pending, total), last errors, and whether indexing is available.
- **Live querying** — `api_query_knowledge_graph` accepts a query string, mode (`local`, `global`, `hybrid`, `naive`, `mix`, `bypass`), and `top_k`. Returns an optional answer plus source nodes and edges.

## Non-Goals (Week 2)

The following are **not** implemented:

- **Live Insight Engine** — No real-time ingestion of transcript chunks during recording. Ingestion only happens post-save via explicit API call.
- **Automatic ingestion** — No hooks in the recording or transcription lifecycle auto-trigger KG ingestion.
- **Graph visualization** — No visual graph explorer of nodes and edges in the UI.
- **Live query during recording** — The query bar only works on already-indexed meetings.
- **Helm chart provisioning** — The chart at `charts/poly/` is a scaffold with placeholder values. It does not provision S3 buckets, IAM roles, or TLS certificates.

## Notes

- Docker is not required for unit tests.
- Use the compose stack only when you want to validate LightRAG, Neo4j, and rustfs together.
- **Automatic live ingestion during recording is future work, not implemented in Week 1 or Week 2.** The KG module only ingests when `api_ingest_meeting_to_knowledge_graph` is explicitly called. No auto-ingestion hooks exist in the recording/transcription lifecycle.
