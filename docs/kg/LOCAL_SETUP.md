# Knowledge Graph Local Setup

This guide walks you through running the LightRAG knowledge-graph stack locally for development and testing. The stack is **completely optional** — Meetily works without it, and KG ingestion is disabled by default.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) + Docker Compose v2
- At least 4 GB of free RAM (LightRAG + Neo4j together)
- (Optional) An OpenAI, Groq, or Ollama API key if you want LLM-powered query responses

## Quick Start

1. **Copy the environment template**

   ```bash
   cp kg.env.example kg.env
   ```

2. **Edit `kg.env` and replace the placeholders**

   ```bash
   # Neo4j credentials
   NEO4J_AUTH=neo4j/your-secure-password

   # RustFS object-store credentials (any random strings work for local dev)
   RUSTFS_ACCESS_KEY=minioadmin
   RUSTFS_SECRET_KEY=minioadmin

   # LightRAG API key (any random string works for local dev)
   LIGHTRAG_API_KEY=dev-key-change-me

   # Embedding model — default is local bge-m3 via mlx
   LIGHTRAG_EMBEDDING_MODEL=bge-m3
   LIGHTRAG_EMBEDDING_MODEL_NAME=BAAI/bge-m3
   ```

   > **Note:** If you change the embedding model after data has been indexed, you must reset the stack (see [Reset](#reset) below). LightRAG does not support mixed embedding spaces.

3. **Start the stack**

   ```bash
   docker compose -f docker-compose.kg.yml --env-file kg.env up -d
   ```

4. **Check health**

   ```bash
   docker compose -f docker-compose.kg.yml ps
   ```

   All three services (`rustfs`, `neo4j`, `lightrag`) should show `healthy` or `running`.

5. **Verify LightRAG is reachable**

   ```bash
   curl http://localhost:9621/health
   ```

   Expected: `{"status":"healthy"}` (or similar JSON).

## Services & Ports

| Service | Port | Purpose |
|---------|------|---------|
| rustfs  | 9000 / 9001 | Object storage (S3-compatible) for document blobs |
| neo4j   | 7687 / 7474 | Graph database for entity/relationship storage |
| lightrag| 9621          | LightRAG API server — ingestion & query endpoint |

## Meetily Integration

1. Open Meetily **Settings** → **Knowledge Graph**
2. Add a profile:
   - **Name:** `local` (or any label)
   - **LightRAG URL:** `http://localhost:9621`
   - **API Key:** the same value you set for `LIGHTRAG_API_KEY` in `kg.env`
3. Save and select the profile
4. After a meeting is recorded, open the meeting and click **"Index to Knowledge Graph"** to explicitly ingest it

> **KG is opt-in.** No meeting is indexed automatically. The app remains fully functional without the KG stack running.

## Daily Commands

```bash
# Start
docker compose -f docker-compose.kg.yml --env-file kg.env up -d

# View logs
docker compose -f docker-compose.kg.yml logs -f

# Stop (keeps data)
docker compose -f docker-compose.kg.yml down

# Stop and delete all data (irreversible)
docker compose -f docker-compose.kg.yml down -v
```

## Reset

If you change the embedding model, or want to wipe all indexed data:

```bash
docker compose -f docker-compose.kg.yml down -v
docker compose -f docker-compose.kg.yml --env-file kg.env up -d
```

This recreates the named volumes from scratch.

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `port 9000 already in use` | Change the host port in `docker-compose.kg.yml` or stop the conflicting service |
| LightRAG logs show `Connection refused` to Neo4j | Wait 10–20 s after `neo4j` starts before LightRAG first connects; restart LightRAG container if needed |
| `kg.env` not loaded | Make sure you pass `--env-file kg.env` (not `kg.env.example`) to every `docker compose` command |
| Queries return empty | Confirm a meeting was explicitly ingested via the **Index to Knowledge Graph** button; ingestion is not automatic |

## Architecture

```
Meetily Desktop App
   └── Knowledge Graph Module
        └── LightRagProvider ──HTTP──> LightRAG (port 9621)
                                              │
                                              ├──> Neo4j (port 7687)
                                              └──> RustFS (port 9000)
```

For the overall Meetily architecture, see [guides/architecture.md](guides/architecture.md).
