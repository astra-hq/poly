# Knowledge Graph Local Setup

This guide walks you through running the LightRAG knowledge-graph stack locally for development and testing. The stack is **completely optional** — Poly works without it, and KG ingestion is disabled by default.

> **Week 2 scope:** This guide covers the local Docker stack and all UI features implemented in Week 2. The Live Insight Engine (real-time ingestion during recording) is **not** implemented yet.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) + Docker Compose v2
- At least 4 GB of free RAM (LightRAG + Neo4j together)
- (Optional) An OpenAI, Groq, or Ollama API key if you want LLM-powered query responses

## Quick Start

1. **Auto-setup (recommended)**

   Open **Settings** → **Knowledge Graph** in the Poly app and add a **Local** profile. The app automatically creates `~/.poly/docker/`, copies the compose template, and generates `kg.env` with sensible defaults.

   If you prefer to set up the files manually, create `~/.poly/docker/` and place `docker-compose.kg.yml` and `kg.env` there.

2. **Edit `~/.poly/docker/kg.env` and replace the placeholders**

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
   docker compose -f ~/.poly/docker/docker-compose.kg.yml --env-file ~/.poly/docker/kg.env up -d
   ```

4. **Check health**

   ```bash
   docker compose -f ~/.poly/docker/docker-compose.kg.yml ps
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

## Poly Integration

### Settings — Knowledge Graph Profiles

Open **Settings** → **Knowledge Graph** to manage LightRAG profiles.

**What you can do:**

- **Add a profile** — Click **Add Profile**. Enter a display name, LightRAG URL (for example `http://localhost:9621`), and optional API key. Choose **Local** or **Remote** kind. You can also set embedding configuration (provider, model, dimensions).
- **Edit a profile** — Click the pencil icon next to any profile.
- **Delete a profile** — Click the trash icon. If the profile was set as default, the default resets to **None**.
- **Test connectivity** — Click **Test** on any profile to run a health check against its LightRAG endpoint. Results show as Healthy, Unreachable, or Not tested.
- **Set a default** — Toggle the **Default** switch on a profile. This profile becomes the suggested choice in the pre-recording selector, but you still must explicitly opt in every time.

> **Important:** The default profile is always **None** until you explicitly set one. Even with a default, no meeting is indexed automatically.

### Pre-Recording Opt-In Selector

Before you start recording, a **Knowledge Graph** selector appears near the recording controls (only when you are not already recording).

**Behavior:**

- The selector defaults to **off** (None). You must explicitly toggle **Use Knowledge Graph** to opt in.
- When you opt in, the first available profile is auto-selected, but you can change it via the dropdown.
- A suggestion text appears based on your meeting name (for example "This looks like a standup — would you like to use a Knowledge Graph?"). This is advisory only and never auto-selects.
- If no profiles are configured, a warning message tells you to add one in Settings.
- Your selection is persisted only after the meeting is saved. It does not trigger any indexing during recording.

### Meeting Detail — Index to Knowledge Graph

After a meeting is recorded and saved, open the meeting detail page. Two KG components appear below the transcript and summary:

#### Knowledge Graph Panel

A collapsible panel labeled **Knowledge Graph** shows:

- **Profile selection** — If no profile was chosen pre-recording, pick one from the available profiles.
- **Index to Knowledge Graph** button — Click to explicitly ingest the meeting transcript. This submits transcript chunks to LightRAG. The button is disabled when there are no transcripts to index.
- **Status counts** — Submitted, Failed, and Total chunk counts update after each indexing attempt.
- **Retry failed** — If some chunks fail, a **Retry** button appears to re-submit only the failed chunks.
- **Last errors** — Any recent errors are shown in a scrollable list for debugging.

> **Indexing is explicit only.** No meeting is ever indexed automatically, even if you selected a profile before recording.

#### Live KG Query Bar

Below the Knowledge Graph panel is a **Search indexed Knowledge Graph content** bar.

**What you can do:**

- Type a natural-language question about the meeting's indexed content.
- Choose a **query mode** from the dropdown: `local`, `global`, `hybrid` (default), `naive`, `mix`, or `bypass`.
- If you have multiple profiles, a profile selector appears next to the mode dropdown.
- Press **Enter** or click **Search** to run the query.

**Results display:**

- **Answer** — A generated answer from LightRAG (if available).
- **Source counts** — Number of source nodes and edges that contributed to the answer.
- **Empty result** — If nothing matches, a "No results found" message appears with a suggestion to try a different query or mode.
- **Error state** — Query failures are shown inline with the error message.

The query bar is disabled when no profile is selected or when the meeting has no indexed chunks yet.

## What is NOT Implemented (Week 2)

The following features are **not** part of the Week 2 release:

- **Live Insight Engine** — Real-time ingestion of transcript chunks during recording. Indexing only happens after the meeting is saved, via the explicit **Index to Knowledge Graph** button.
- **Automatic ingestion** — No meeting is ever indexed without explicit user action.
- **Graph visualization** — No visual graph explorer of nodes and edges.
- **Live query during recording** — The query bar only works on already-indexed meetings.

## Daily Commands

```bash
# Start
docker compose -f ~/.poly/docker/docker-compose.kg.yml --env-file ~/.poly/docker/kg.env up -d

# View logs
docker compose -f ~/.poly/docker/docker-compose.kg.yml logs -f

# Stop (keeps data)
docker compose -f ~/.poly/docker/docker-compose.kg.yml down

# Stop and delete all data (irreversible)
docker compose -f ~/.poly/docker/docker-compose.kg.yml down -v
```

## Reset

If you change the embedding model, or want to wipe all indexed data:

```bash
docker compose -f ~/.poly/docker/docker-compose.kg.yml down -v
docker compose -f ~/.poly/docker/docker-compose.kg.yml --env-file ~/.poly/docker/kg.env up -d
```

This recreates the named volumes from scratch.

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `port 9000 already in use` | Change the host port in `~/.poly/docker/docker-compose.kg.yml` or stop the conflicting service |
| LightRAG logs show `Connection refused` to Neo4j | Wait 10–20 s after `neo4j` starts before LightRAG first connects; restart LightRAG container if needed |
| `kg.env` not loaded | Make sure you pass `--env-file ~/.poly/docker/kg.env` to every `docker compose` command |
| Queries return empty | Confirm a meeting was explicitly ingested via the **Index to Knowledge Graph** button; ingestion is not automatic |

## Architecture

```
Poly Desktop App
   └── Knowledge Graph Module
        └── LightRagProvider --HTTP--> LightRAG (port 9621)
                                              |
                                              |--> Neo4j (port 7687)
                                              |--> RustFS (port 9000)
```

For the overall Poly architecture, see [../guides/architecture.md](../guides/architecture.md).
