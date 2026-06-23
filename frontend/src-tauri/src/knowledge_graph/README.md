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

## Notes

- Docker is not required for unit tests.
- Use the compose stack only when you want to validate LightRAG, Neo4j, and rustfs together.
- **Automatic live ingestion during recording is future work, not implemented in Week 1.** The KG module only ingests when `api_ingest_meeting_to_knowledge_graph` is explicitly called. No auto-ingestion hooks exist in the recording/transcription lifecycle.
