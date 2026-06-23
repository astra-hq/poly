# Resourcefully Knowledge Graph Foundation

## TL;DR
> **Summary**: Build the Week 1 foundation for Resourcefully’s in-app Knowledge Graph Interface: configurable KG profiles, Rust provider abstraction, LightRAG client, transcript chunking, recorded-meeting ingestion, and a local KG stack. Keep all KG ingestion opt-in and failure-isolated from recording/transcription.
> **Deliverables**:
> - `frontend/src-tauri/src/knowledge_graph/` module with trait/types/config/chunker/LightRAG client/mock provider
> - DB/settings-backed KG profile config with default `None`
> - Explicit recorded-meeting ingestion command using LightRAG `POST /documents/text`
> - Local `docker-compose.kg.yml` + KG `.env.example`
> - Rust/Bun tests and agent-executed QA evidence
> **Effort**: Medium
> **Parallel**: YES - 6 waves
> **Critical Path**: Task 1 → Task 2 → Task 3 → Tasks 4/5 → Task 6 → Task 8

## Context
### Original Request
- User asked to get familiar with `/Users/hermes/workspace/mind/Ideas/Meetingily*` and plan work around the Meetingily knowledge graph interface.
- Exact `Meetingily` spelling had no matches; relevant vault docs use `Meetily`, `Resourcefully`, and `Knowledge Graph Interface`.

### Interview Summary
- Scope selected: **Week 1 foundation only**.
- Embeddings selected: **make configurable**, default to local **bge-m3**, and document that changing embedding model/provider after indexing requires re-indexing/reset.
- Test strategy selected: **tests-after + agent-executed QA**, no Playwright setup unless a task adds real UI workflow.

### Metis Review (gaps addressed)
- Added LightRAG API validation before implementation tasks depend on endpoints.
- Kept Week 1 away from graph visualization and Insight Engine work.
- Required mock-provider/client tests before live Docker integration.
- Made KG disabled-by-default and explicit opt-in for ingestion.
- Required KG failures to never fail meeting recording/transcription persistence.
- Added re-index/embedding immutability guardrail.

## Work Objectives
### Core Objective
Create a safe, testable KG ingestion foundation inside the supported Tauri/Rust app so recorded meetings can be explicitly indexed into LightRAG through a provider abstraction.

### Deliverables
- Rust `knowledge_graph` module under `frontend/src-tauri/src/knowledge_graph/`.
- Config/profile storage integrated with existing Tauri app settings patterns.
- `LightRagProvider` HTTP client targeting verified LightRAG endpoints:
  - `GET /health`
  - `POST /documents/text`
  - `POST /query`
  - `GET /documents/pipeline_status`
  - `GET /documents/track_status/{track_id}`
  - Auth header: `X-API-Key` when configured.
- `TranscriptChunker` for stored transcripts using time-window chunking with final flush.
- Explicit recorded-meeting ingestion command: `api_ingest_meeting_to_knowledge_graph`.
- Local ingestion ledger table(s) for idempotent per-profile/per-meeting/per-chunk submissions.
- Local KG stack file: `docker-compose.kg.yml` at repo root, not replacing any existing compose file.
- KG env template: `kg.env.example` at repo root.

### Definition of Done (verifiable conditions with commands)
- `cd frontend/src-tauri && cargo test knowledge_graph` passes.
- `cd frontend/src-tauri && cargo test` passes or failures are documented as pre-existing with exact failing tests.
- `cd frontend && bun test tests/lib` passes.
- `docker compose -f docker-compose.kg.yml config` succeeds.
- If Docker is available: `docker compose -f docker-compose.kg.yml --env-file kg.env.example up -d` reaches healthy status for LightRAG/Neo4j/rustfs or failure evidence identifies missing local dependencies only.
- A mock-provider test proves KG disabled/default `None` causes zero provider calls.
- A mock-provider test proves provider failure does not fail recorded-meeting retrieval or transcript persistence.

### Must Have
- Use the supported Tauri app only: `frontend/` and `frontend/src-tauri/src/`.
- Keep archived `backend/` out of scope.
- Ingestion must require explicit profile selection by command argument; default profile is `None`.
- Store KG profiles in existing settings infrastructure unless implementation proves it cannot support JSON config.
- Include mock provider for deterministic tests.
- Use a local SQLite ingestion ledger to prevent duplicate submissions; do not rely on LightRAG duplicate behavior.
- Include timeout/error handling for LightRAG unavailable, 500, malformed response, and missing credentials.
- Preserve transcript unicode/non-English text.

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- No graph visualization UI.
- No Insight Engine, knowledge-gap prompts, action items, decision capture, relationship synthesis.
- No Obsidian sync.
- No Helm chart or remote deployment.
- No automatic indexing during recording.
- No FastAPI/Python backend changes.
- No Playwright setup.
- No hard-coded local absolute paths, secrets, or API keys.
- No embedding model changes after indexed data exists without a warning/reset path.

## Verification Strategy
> ZERO HUMAN INTERVENTION - all verification is agent-executed.
- Test decision: tests-after + Rust Cargo tests + existing Bun tests. No E2E setup.
- QA policy: Every task has agent-executed scenarios.
- Evidence: `.omo/evidence/task-{N}-{slug}.{ext}`

## Execution Strategy
### Parallel Execution Waves
> Target: 5-8 tasks per wave. <3 per wave (except final) = under-splitting.
> Extract shared dependencies as Wave-1 tasks for max parallelism.

Wave 1: Task 1 (contract/version), Task 7 (local compose/env)
Wave 2: Task 2 (config)
Wave 3: Task 3 (trait/types/mock)
Wave 4: Task 4 (LightRAG client), Task 5 (chunker), Task 9 (test fixtures/helpers)
Wave 5: Task 6 (recorded ingestion command)
Wave 6: Task 8 (disabled-by-default integration/QA hardening)

### Dependency Matrix (full, all tasks)
- Task 1 blocks Task 2.
- Task 2 blocks Tasks 3, 6, 8.
- Task 3 blocks Tasks 4, 5, 6, 8, 9.
- Task 4 blocks Task 6.
- Task 5 blocks Task 6.
- Task 7 blocks live compose QA only; it does not block mock-based tests or Rust implementation.
- Task 9 supports Tasks 4, 5, 6, 8.
- Task 6 blocks Task 8.
- Final verification blocks completion.

### Agent Dispatch Summary (wave → task count → categories)
- Wave 1 → 2 tasks → `quick`, `unspecified-high`
- Wave 2 → 1 task → `unspecified-high`
- Wave 3 → 1 task → `unspecified-high`
- Wave 4 → 3 tasks → `unspecified-high`, `quick`
- Wave 5 → 1 task → `unspecified-high`
- Wave 6 → 1 task → `unspecified-high`
- Final Verification → 4 review tasks → oracle / unspecified-high / unspecified-high / deep

## TODOs
> Implementation + Test = ONE task. Never separate.
> EVERY task MUST have: Agent Profile + Parallelization + QA Scenarios.

- [x] 1. Validate and pin LightRAG Week-1 API contract

  **What to do**: Add a short source-controlled implementation note at `frontend/src-tauri/src/knowledge_graph/API_CONTRACT.md` documenting the exact LightRAG contract that implementation must target. Include commit `4e1f95269b32cd803cb96e0acc24d922aa6b6932`, endpoints, auth header, request/response fields needed by Week 1, and version caveat that v1.5+ behavior changed. Create the `knowledge_graph/` directory if absent, but add no executable Rust logic in this task.
  **Must NOT do**: Do not use the old sketch’s `GET /query` form; verified query is `POST /query`. Do not copy large docs or vendor LightRAG code.

  **Recommended Agent Profile**:
  - Category: `quick` - Reason: bounded documentation artifact based on already verified official docs.
  - Skills: `[]` - no special skill required.
  - Omitted: `security-research` - no vulnerability audit needed.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 2 | Blocked By: none

  **References**:
  - Vault spec: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:142-160` - original LightRAG provider sketch; supersede only the unverified GET query shape.
  - Official LightRAG source: `https://github.com/HKUDS/LightRAG/blob/4e1f95269b32cd803cb96e0acc24d922aa6b6932/lightrag/api/routers/document_routes.py#L2812-L2903` - `POST /documents/text`.
  - Official LightRAG source: `https://github.com/HKUDS/LightRAG/blob/4e1f95269b32cd803cb96e0acc24d922aa6b6932/lightrag/api/routers/query_routes.py#L200-L451` - `POST /query`.
  - Official LightRAG source: `https://github.com/HKUDS/LightRAG/blob/4e1f95269b32cd803cb96e0acc24d922aa6b6932/lightrag/api/lightrag_server.py#L2196-L2299` - `GET /health`.
  - Official LightRAG docs: `https://github.com/HKUDS/LightRAG/blob/4e1f95269b32cd803cb96e0acc24d922aa6b6932/docs/LightRAG-API-Server.md#L590-L603` - `X-API-Key` auth.

  **Acceptance Criteria**:
  - [ ] `test -f frontend/src-tauri/src/knowledge_graph/API_CONTRACT.md` exits 0.
  - [ ] `grep -q "POST /documents/text" frontend/src-tauri/src/knowledge_graph/API_CONTRACT.md` exits 0.
  - [ ] `grep -q "POST /query" frontend/src-tauri/src/knowledge_graph/API_CONTRACT.md` exits 0.
  - [ ] `grep -q "X-API-Key" frontend/src-tauri/src/knowledge_graph/API_CONTRACT.md` exits 0.
  - [ ] `grep -q "4e1f95269b32cd803cb96e0acc24d922aa6b6932" frontend/src-tauri/src/knowledge_graph/API_CONTRACT.md` exits 0.

  **QA Scenarios**:
  ```
  Scenario: Contract doc pins verified endpoints
    Tool: Bash
    Steps: cd frontend/src-tauri && grep -E "POST /documents/text|POST /query|GET /health|X-API-Key" src/knowledge_graph/API_CONTRACT.md
    Expected: All four tokens appear in command output.
    Evidence: .omo/evidence/task-1-api-contract.txt

  Scenario: Contract doc rejects stale query shape
    Tool: Bash
    Steps: cd frontend/src-tauri && ! grep -q "GET /query" src/knowledge_graph/API_CONTRACT.md
    Expected: Command exits 0 because stale GET query shape is absent.
    Evidence: .omo/evidence/task-1-api-contract-error.txt
  ```

  **Commit**: NO | Message: `docs(kg): pin lightrag api contract` | Files: `frontend/src-tauri/src/knowledge_graph/API_CONTRACT.md`

- [x] 2. Add KG profile configuration with disabled default

  **What to do**: Implement KG config types and settings persistence using existing settings patterns. Add `KnowledgeGraphSettings`, `KnowledgeGraphProfile`, `EmbeddingConfig`, and `KnowledgeGraphSelection` types under `frontend/src-tauri/src/knowledge_graph/config.rs`. Persist profiles as JSON in the existing settings repository using keys `knowledge_graph_profiles`, `knowledge_graph_default_selection`, and `knowledge_graph_embedding_fingerprint`. Default selection must deserialize to `None`. Default `EmbeddingConfig` must resolve to local provider `mlx` with model `BAAI/bge-m3` (or exact internal model ID `bge-m3` if the app’s config style stores short names). Include helper functions to load profiles, resolve a profile by ID, validate endpoint/API key presence, and detect embedding fingerprint changes. Add/update `frontend/src-tauri/src/knowledge_graph/mod.rs` only enough to export `config` so Task 2 tests run independently; Task 3 may extend `mod.rs` later for provider/types exports.
  **Must NOT do**: Do not create a new DB table unless the existing settings repository cannot store JSON; if a new table becomes necessary, document the blocker in `.omo/evidence/task-2-config-deviation.md` before implementing. Do not store real API keys in tests.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: touches Rust config types and persistent settings behavior.
  - Skills: `[]` - no special skill required.
  - Omitted: `security-research` - secrets are represented by dummy values only.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 3, 6, 8 | Blocked By: 1

  **References**:
  - Vault decision: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:231-250` - opt-in default None and provider config fields.
  - Vault decision: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:161-177` - model roles and embedding immutability.
  - Existing settings pattern: `frontend/src-tauri/src/database/repositories/setting.rs` - use existing setting persistence.
  - Existing models: `frontend/src-tauri/src/database/models.rs` - follow serde/sqlx model style.

  **Acceptance Criteria**:
  - [ ] `cd frontend/src-tauri && cargo test knowledge_graph::config` passes.
  - [ ] Tests prove missing config loads as disabled/default `None`.
  - [ ] Tests prove default embedding config resolves to local MLX + bge-m3.
  - [ ] Tests prove dummy profile validates only with non-empty endpoint and provider type `lightrag`.
  - [ ] Tests prove changed embedding fingerprint returns a re-index/reset warning state, not silent success.

  **QA Scenarios**:
  ```
  Scenario: Default KG config is disabled
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::config::tests::default_selection_is_none -- --nocapture
    Expected: Test passes and output states default selection is None/disabled.
    Evidence: .omo/evidence/task-2-config.txt

  Scenario: Embedding change requires explicit reset
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::config::tests::embedding_fingerprint_change_requires_reindex -- --nocapture
    Expected: Test passes and returns a re-index/reset warning result.
    Evidence: .omo/evidence/task-2-config-error.txt
  ```

  **Commit**: NO | Message: `feat(kg): add disabled-by-default profile config` | Files: `frontend/src-tauri/src/knowledge_graph/config.rs`, `frontend/src-tauri/src/knowledge_graph/mod.rs`

- [x] 3. Add provider trait, shared types, and mock provider

  **What to do**: Implement `KnowledgeGraphProvider` abstraction and shared request/response types under `frontend/src-tauri/src/knowledge_graph/`. Use async trait-compatible design if current Rust dependencies support it; otherwise use boxed futures consistently. Required methods: `health`, `insert_chunk`, `query`, `pipeline_status`, `track_status`, `name`, `config_id`. Add `MockKnowledgeGraphProvider` behind `#[cfg(test)]` or test module for deterministic tests. Include types for `TranscriptChunk`, `QueryMode`, `QueryResponse`, `InsertResponse`, `ProviderError`, and `ProviderResult`. Extend `frontend/src-tauri/src/knowledge_graph/mod.rs` with provider/types exports created in this task.
  **Must NOT do**: Do not implement LightRAG HTTP calls in this task. Do not expose UI commands here.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: establishes core abstraction used by all later tasks.
  - Skills: `[]` - no special skill required.
  - Omitted: `security-research` - no security audit needed for trait scaffolding.

  **Parallelization**: Can Parallel: NO | Wave 3 | Blocks: 4, 5, 6, 8, 9 | Blocked By: 2

  **References**:
  - Vault trait sketch: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:113-140` - baseline provider and chunk types.
  - Vault hook plan: `/Users/hermes/workspace/mind/Ideas/Meetily-Codebase-Deep-Dive.md:230-248` - proposed `knowledge_graph/` module.
  - Existing app style: `frontend/src-tauri/src/summary/commands.rs` - result/error mapping for Tauri-facing operations.
  - Existing app style: `frontend/src-tauri/src/audio/transcription/worker.rs` - transcript metadata source fields.

  **Acceptance Criteria**:
  - [ ] `cd frontend/src-tauri && cargo test knowledge_graph::provider` passes.
  - [ ] `QueryMode` serializes/deserializes exact LightRAG modes: `local`, `global`, `hybrid`, `naive`, `mix`, `bypass`.
  - [ ] Mock provider records insert/query calls and can simulate timeout/unavailable/provider-error results.
  - [ ] `TranscriptChunk` includes meeting ID, sequence, text, optional speaker, start/end timestamps, and chunk source/type.

  **QA Scenarios**:
  ```
  Scenario: Provider trait supports deterministic mock insert/query
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::provider::tests::mock_provider_records_calls -- --nocapture
    Expected: Test passes with one insert call and one query call recorded.
    Evidence: .omo/evidence/task-3-provider.txt

  Scenario: Provider errors are typed and non-panicking
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::provider::tests::mock_provider_returns_typed_error -- --nocapture
    Expected: Test passes and typed ProviderError is returned without panic.
    Evidence: .omo/evidence/task-3-provider-error.txt
  ```

  **Commit**: NO | Message: `feat(kg): add provider trait and shared types` | Files: `frontend/src-tauri/src/knowledge_graph/provider.rs`, `frontend/src-tauri/src/knowledge_graph/types.rs`, `frontend/src-tauri/src/knowledge_graph/mod.rs`

- [x] 4. Implement LightRAG HTTP client against verified API

  **What to do**: Implement `LightRagProvider` in `frontend/src-tauri/src/knowledge_graph/lightrag.rs` using `reqwest` only if not already present; otherwise reuse existing HTTP dependency patterns. Methods must call verified endpoints: `GET /health`, `POST /documents/text`, `POST /query`, `GET /documents/pipeline_status`, and `GET /documents/track_status/{track_id}`. Add `X-API-Key` only when configured. Use finite timeouts. Serialize inserted chunks as text with a stable header containing meeting ID, chunk sequence, timestamps, optional speaker, and chunk source, and set `file_source` to `resourcefully/meetings/{meeting_id}/chunks/{sequence}.txt`.
  **Must NOT do**: Do not call external services in unit tests. Do not use GET for query. Do not include real API keys.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: external HTTP integration with error mapping and mocks.
  - Skills: `[]` - no special skill required.
  - Omitted: `security-research` - auth is simple API-key client behavior, not audit scope.

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 6 | Blocked By: 1, 3

  **References**:
  - Contract doc from Task 1: `frontend/src-tauri/src/knowledge_graph/API_CONTRACT.md`.
  - Official insert endpoint: `https://github.com/HKUDS/LightRAG/blob/4e1f95269b32cd803cb96e0acc24d922aa6b6932/lightrag/api/routers/document_routes.py#L2812-L2903`.
  - Official query endpoint: `https://github.com/HKUDS/LightRAG/blob/4e1f95269b32cd803cb96e0acc24d922aa6b6932/lightrag/api/routers/query_routes.py#L200-L451`.
  - Existing HTTP client style: `frontend/src-tauri/src/summary/llm_client.rs` - follow timeout/error conventions if present.
  - Cargo manifest: `frontend/src-tauri/Cargo.toml` - add dependency only if required.

  **Acceptance Criteria**:
  - [ ] `cd frontend/src-tauri && cargo test knowledge_graph::lightrag` passes using mock HTTP server or dependency-injected transport.
  - [ ] Insert test asserts method `POST`, path `/documents/text`, JSON body contains `text` and `file_source`.
  - [ ] Query test asserts method `POST`, path `/query`, and mode serializes as `hybrid` for hybrid query.
  - [ ] Auth test asserts `X-API-Key` is present only when profile has an API key.
  - [ ] Failure tests cover timeout, HTTP 500, malformed JSON, and unavailable server.

  **QA Scenarios**:
  ```
  Scenario: LightRAG insert chunk sends verified request shape
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::lightrag::tests::insert_chunk_posts_documents_text -- --nocapture
    Expected: Test passes and mock server records POST /documents/text with text + file_source.
    Evidence: .omo/evidence/task-4-lightrag.txt

  Scenario: LightRAG provider maps server failure without panic
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::lightrag::tests::insert_chunk_maps_500_to_provider_error -- --nocapture
    Expected: Test passes and returns ProviderError::Http or equivalent typed error.
    Evidence: .omo/evidence/task-4-lightrag-error.txt
  ```

  **Commit**: NO | Message: `feat(kg): implement lightrag provider client` | Files: `frontend/src-tauri/src/knowledge_graph/lightrag.rs`, `frontend/src-tauri/Cargo.toml`, `frontend/src-tauri/Cargo.lock`

- [x] 5. Implement stored-transcript chunker for recorded meetings

  **What to do**: Implement `TranscriptChunker` in `frontend/src-tauri/src/knowledge_graph/chunker.rs`. It must accept stored transcript rows for one meeting and emit deterministic chunks using configurable `chunk_interval_secs` default `20`, min `5`, max `120`. Use transcript `audio_start_time`/`audio_end_time` where available. Flush final partial chunk. Preserve transcript order by timestamp then sequence/id. Handle missing speaker as `None`, empty text by skipping, duplicate transcript IDs by ingesting once, out-of-order rows by sorting, and non-English unicode text unchanged.
  **Must NOT do**: Do not implement topic-based chunking in Week 1. Do not modify audio capture/VAD/transcription worker in this task.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: core deterministic data transformation with edge cases.
  - Skills: `[]` - no special skill required.
  - Omitted: `visual-engineering` - no UI work.

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 6 | Blocked By: 3

  **References**:
  - Vault chunking requirement: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:46-50` - chunk every 15-30s or speaker turn.
  - Vault type sketch: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:130-137` - chunk fields.
  - Existing transcript model: `frontend/src-tauri/src/database/models.rs` - source row fields.
  - Existing transcript repo: `frontend/src-tauri/src/database/repositories/transcript.rs` - retrieval/search patterns.

  **Acceptance Criteria**:
  - [ ] `cd frontend/src-tauri && cargo test knowledge_graph::chunker` passes.
  - [ ] 20s window test emits expected chunk count and start/end times.
  - [ ] Empty transcript text is ignored.
  - [ ] Final partial chunk flushes.
  - [ ] Out-of-order transcript rows produce ordered chunks.
  - [ ] Unicode text remains byte-for-byte equivalent in emitted chunk text.

  **QA Scenarios**:
  ```
  Scenario: Chunker emits deterministic 20-second windows
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::chunker::tests::chunks_by_twenty_second_windows -- --nocapture
    Expected: Test passes with deterministic chunk sequences and timestamps.
    Evidence: .omo/evidence/task-5-chunker.txt

  Scenario: Chunker skips invalid empty rows and flushes final partial
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::chunker::tests::skips_empty_and_flushes_final_partial -- --nocapture
    Expected: Test passes; no empty chunks emitted and final partial chunk exists.
    Evidence: .omo/evidence/task-5-chunker-error.txt
  ```

  **Commit**: NO | Message: `feat(kg): add transcript chunker` | Files: `frontend/src-tauri/src/knowledge_graph/chunker.rs`

- [x] 6. Add explicit recorded-meeting ingestion command

  **What to do**: Implement service/command path for explicit recorded-meeting ingestion. Add `frontend/src-tauri/src/knowledge_graph/service.rs` and `commands.rs`. Register `api_ingest_meeting_to_knowledge_graph` in `frontend/src-tauri/src/lib.rs`. Command input must include `meeting_id` and `profile_id`; reject missing/`None` profile. Load meeting transcripts from existing repositories, chunk them, insert each chunk through the resolved provider, collect per-chunk results/track IDs, and return an ingestion summary. Add a local SQLite ingestion ledger via existing database migration/setup patterns with table name `knowledge_graph_ingestion_chunks` and unique key `(meeting_id, profile_id, chunk_sequence, chunk_fingerprint)`. Ledger columns must include `meeting_id`, `profile_id`, `chunk_sequence`, `chunk_fingerprint`, `file_source`, `status`, `track_id`, `last_error`, `created_at`, and `updated_at`. Before provider calls, skip ledger rows with `status='submitted'` or `status='completed'`; return them as `already_submitted`. For failed rows, retry only that chunk and update `last_error`. Do not rely on LightRAG duplicate/conflict behavior for idempotency.
  **Must NOT do**: Do not automatically run this command after recording stop. Do not change recording/transcription success behavior. Do not build UI.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: combines DB reads, config, chunking, provider calls, and Tauri command registration.
  - Skills: `[]` - no special skill required.
  - Omitted: `visual-engineering` - no UI is in Week 1.

  **Parallelization**: Can Parallel: NO | Wave 5 | Blocks: 8 | Blocked By: 2, 3, 4, 5, 9

  **References**:
  - Vault implementation order: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:361-368` - basic chunk→KG pipeline with recorded meetings first.
  - App command registration: `frontend/src-tauri/src/lib.rs` - register Tauri commands here.
  - DB API patterns: `frontend/src-tauri/src/api/api.rs` - app/DB command naming and error shape.
  - Meeting repo: `frontend/src-tauri/src/database/repositories/meeting.rs` - meeting retrieval patterns.
  - Transcript repo: `frontend/src-tauri/src/database/repositories/transcript.rs` - transcript retrieval/search patterns.
  - DB setup/migrations: `frontend/src-tauri/src/database/manager.rs`, `frontend/src-tauri/src/database/setup.rs` - follow existing SQLite initialization patterns.

  **Acceptance Criteria**:
  - [ ] `cd frontend/src-tauri && cargo test knowledge_graph::service` passes.
  - [ ] Command rejects `profile_id="none"` or missing profile with no provider calls.
  - [ ] Command returns structured error for missing meeting ID.
  - [ ] Mock-provider test ingests chunks in sequence and returns count/track IDs.
  - [ ] Duplicate command invocation skips previously submitted chunks using `knowledge_graph_ingestion_chunks` and returns `already_submitted` count.
  - [ ] Failed ledger rows are retried without re-sending successful chunks.
  - [ ] Mock-provider failure returns partial summary without corrupting meeting/transcript data.
  - [ ] `api_ingest_meeting_to_knowledge_graph` is registered in `tauri::generate_handler!`.

  **QA Scenarios**:
  ```
  Scenario: Recorded meeting ingests through mock provider
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::service::tests::ingests_recorded_meeting_chunks_in_order -- --nocapture
    Expected: Test passes; mock provider receives ordered chunks and command returns submitted count.
    Evidence: .omo/evidence/task-6-ingestion.txt

  Scenario: Disabled profile prevents ingestion
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::service::tests::none_profile_sends_zero_provider_calls -- --nocapture
    Expected: Test passes; provider call count is zero and result is rejected/disabled.
    Evidence: .omo/evidence/task-6-ingestion-error.txt

  Scenario: Duplicate ingestion uses local ledger
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::service::tests::duplicate_ingestion_skips_submitted_chunks -- --nocapture
    Expected: Test passes; second invocation sends zero provider calls for previously submitted chunks and returns already_submitted count.
    Evidence: .omo/evidence/task-6-ingestion-idempotency.txt
  ```

  **Commit**: NO | Message: `feat(kg): add explicit meeting ingestion command` | Files: `frontend/src-tauri/src/knowledge_graph/service.rs`, `frontend/src-tauri/src/knowledge_graph/commands.rs`, `frontend/src-tauri/src/lib.rs`, `frontend/src-tauri/src/database/manager.rs`, `frontend/src-tauri/src/database/setup.rs`

- [x] 7. Add local KG compose stack and env template

  **What to do**: Add root `docker-compose.kg.yml` and `kg.env.example` for local development. Use services `rustfs`, `neo4j`, and `lightrag`. Include persistent named volumes, ports from the vault doc, health checks where images support them, and comments explaining required LLM/embedding variables. Default embedding docs must point to configurable local bge-m3 while allowing remote provider override. Use placeholder secrets only.
  **Must NOT do**: Do not replace root `docker-compose.yml` if one exists. Do not add real credentials. Do not add remote Helm chart.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: infra config must be valid and safe by default.
  - Skills: `[]` - no special skill required.
  - Omitted: `security-research` - no security audit, but avoid secrets.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: live compose QA | Blocked By: none

  **References**:
  - Vault local stack: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:52-67` - rustfs, Neo4j, LightRAG and ports.
  - Vault model config: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:161-187` - configurable MLX/remote model roles.
  - Official Docker docs: `https://github.com/HKUDS/LightRAG/blob/4e1f95269b32cd803cb96e0acc24d922aa6b6932/docs/LightRAG-API-Server.md#L212-L226` - copy env and compose/run pattern.

  **Acceptance Criteria**:
  - [ ] `test -f docker-compose.kg.yml` exits 0.
  - [ ] `test -f kg.env.example` exits 0.
  - [ ] `docker compose -f docker-compose.kg.yml --env-file kg.env.example config` succeeds.
  - [ ] `grep -q "LIGHTRAG_API_KEY" kg.env.example` exits 0.
  - [ ] `grep -q "bge-m3" kg.env.example` exits 0.
  - [ ] No real secret-looking values are committed.

  **QA Scenarios**:
  ```
  Scenario: Compose config renders successfully
    Tool: Bash
    Steps: docker compose -f docker-compose.kg.yml --env-file kg.env.example config
    Expected: Command exits 0 and lists rustfs, neo4j, and lightrag services.
    Evidence: .omo/evidence/task-7-compose.txt

  Scenario: Env template contains placeholders only
    Tool: Bash
    Steps: grep -E "CHANGE_ME|example|placeholder" kg.env.example && ! grep -E "sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}" kg.env.example
    Expected: Placeholder text exists and common real-secret patterns are absent.
    Evidence: .omo/evidence/task-7-compose-error.txt
  ```

  **Commit**: NO | Message: `chore(kg): add local lightrag compose stack` | Files: `docker-compose.kg.yml`, `kg.env.example`

- [x] 8. Harden disabled-by-default and failure-isolation behavior

  **What to do**: Add integration tests and minimal wiring to ensure KG foundation cannot break recording/transcription. Verify default config disables ingestion, explicit command is the only ingestion path, provider failures are logged/returned in ingestion summary only, and no recording lifecycle command automatically indexes. If Task 6 introduced command registration, add tests around command-level guard behavior. Update inline module docs to state automatic live ingestion is future work, not Week 1.
  **Must NOT do**: Do not add pre-recording selection UI or recording-stop hooks in Week 1. Do not modify audio pipeline hot path.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: cross-cutting safety and regression tests.
  - Skills: `[]` - no special skill required.
  - Omitted: `visual-engineering` - no UI work.

  **Parallelization**: Can Parallel: NO | Wave 6 | Blocks: final verification | Blocked By: 2, 3, 6

  **References**:
  - Vault opt-in decision: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:399-426` - resolved decisions, especially opt-in and live streaming as future foundation.
  - Recording flow: `frontend/src-tauri/src/audio/recording_commands.rs` - ensure no automatic KG calls are added here.
  - Transcription worker: `frontend/src-tauri/src/audio/transcription/worker.rs` - do not hook live ingestion in Week 1.
  - Stop/save flow: `frontend/src/hooks/useRecordingStop.ts` - no frontend auto-ingestion.

  **Acceptance Criteria**:
  - [ ] `cd frontend/src-tauri && cargo test knowledge_graph::integration` passes.
  - [ ] Test proves no provider calls happen without explicit command invocation.
  - [ ] Test proves provider failure does not delete/update meeting/transcript rows.
  - [ ] Search confirms `recording_commands.rs` does not reference `knowledge_graph` unless only behind comments explicitly stating future work.
  - [ ] Search confirms `worker.rs` does not call LightRAG/provider directly in Week 1.

  **QA Scenarios**:
  ```
  Scenario: KG cannot run implicitly
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph::integration::tests::kg_does_not_ingest_without_explicit_command -- --nocapture
    Expected: Test passes with zero mock-provider calls.
    Evidence: .omo/evidence/task-8-safety.txt

  Scenario: Recording hot path remains unmodified by KG calls
    Tool: Bash
    Steps: cd frontend/src-tauri && ! grep -R "LightRagProvider\|api_ingest_meeting_to_knowledge_graph" src/audio/recording_commands.rs src/audio/transcription/worker.rs
    Expected: Command exits 0 because KG ingestion is not wired into hot path in Week 1.
    Evidence: .omo/evidence/task-8-safety-error.txt
  ```

  **Commit**: NO | Message: `test(kg): harden opt-in ingestion guardrails` | Files: `frontend/src-tauri/src/knowledge_graph/*`, `frontend/src-tauri/src/audio/recording_commands.rs`, `frontend/src-tauri/src/audio/transcription/worker.rs`

- [x] 9. Add KG test fixtures and command documentation for executor QA

  **What to do**: Add test fixtures under `frontend/src-tauri/src/knowledge_graph/test_fixtures.rs` or module-local test helpers: sample meeting ID, ordered/out-of-order transcript rows, unicode transcript text, duplicate row IDs, and mock LightRAG responses. Add `frontend/src-tauri/src/knowledge_graph/README.md` with local development commands, compose lifecycle, cargo test targets, and explicit note that live ingestion/query UI are future work.
  **Must NOT do**: Do not add large binary/audio fixtures. Do not require Docker for unit tests.

  **Recommended Agent Profile**:
  - Category: `quick` - Reason: fixtures/docs to improve repeatable tests.
  - Skills: `[]` - no special skill required.
  - Omitted: `visual-engineering` - no UI work.

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 6 | Blocked By: 3

  **References**:
  - Test infra finding: `frontend/tests/lib/` - existing Bun tests remain separate.
  - Rust test examples: `frontend/src-tauri/src/summary/service.rs`, `frontend/src-tauri/src/onboarding.rs` - representative inline test style.
  - Vault Week 1 order: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:359-368` - recorded meeting test foundation.

  **Acceptance Criteria**:
  - [ ] `cd frontend/src-tauri && cargo test knowledge_graph` passes using fixtures.
  - [ ] `frontend/src-tauri/src/knowledge_graph/README.md` includes `cargo test knowledge_graph`.
  - [ ] README includes `docker compose -f docker-compose.kg.yml --env-file kg.env.example config`.
  - [ ] Fixtures include unicode and out-of-order transcript examples.

  **QA Scenarios**:
  ```
  Scenario: KG fixtures are used by tests
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph -- --nocapture
    Expected: Test output includes knowledge_graph tests passing with fixture-backed cases.
    Evidence: .omo/evidence/task-9-fixtures.txt

  Scenario: README documents no-Docker unit test path
    Tool: Bash
    Steps: grep -q "cargo test knowledge_graph" frontend/src-tauri/src/knowledge_graph/README.md && grep -q "Docker is not required for unit tests" frontend/src-tauri/src/knowledge_graph/README.md
    Expected: Command exits 0; README explicitly separates unit tests from live compose QA.
    Evidence: .omo/evidence/task-9-fixtures-error.txt
  ```

  **Commit**: NO | Message: `test(kg): add fixtures and qa docs` | Files: `frontend/src-tauri/src/knowledge_graph/test_fixtures.rs`, `frontend/src-tauri/src/knowledge_graph/README.md`

## Final Verification Wave (MANDATORY — after ALL implementation tasks)
> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.
> **Do NOT auto-proceed after verification. Wait for user's explicit approval before marking work complete.**
> **Never mark F1-F4 as checked before getting user's okay.** Rejection or user feedback -> fix -> re-run -> present again -> wait for okay.
- [x] F1. Plan Compliance Audit — oracle

  **What to do**: Verify final implementation against this plan line-by-line. Confirm all nine tasks were completed or explicitly waived by user, all scope exclusions were preserved, all acceptance criteria commands were run, and all evidence files exist.
  **Must NOT do**: Do not fix code; report findings only.
  **Recommended Agent Profile**:
  - Category: `oracle` - Reason: independent plan-compliance reasoning.
  - Skills: `[]` - no special skill required.
  - Omitted: `visual-engineering` - this is not UI review.
  **Parallelization**: Can Parallel: YES | Final Wave | Blocks: completion | Blocked By: 1-9
  **References**:
  - Plan: `.omo/plans/meetingily-knowledge-graph-foundation.md` - source of truth.
  - Evidence: `.omo/evidence/task-*` - required task outputs.
  **Acceptance Criteria**:
  - [ ] Oracle report says APPROVE or lists exact plan deviations.
  - [ ] Report confirms no graph UI, Insight Engine, Helm, Obsidian sync, or archived backend changes.
  **QA Scenarios**:
  ```
  Scenario: Plan compliance audit completes
    Tool: task oracle
    Steps: Review `.omo/plans/meetingily-knowledge-graph-foundation.md` against git diff and `.omo/evidence/task-*`.
    Expected: APPROVE with no critical deviations, or exact deviations for fixes.
    Evidence: .omo/evidence/f1-plan-compliance.md

  Scenario: Scope exclusions preserved
    Tool: Bash
    Steps: git diff --name-only | grep -E "(^backend/|charts/|openspec/|\.specify/)"; test $? -ne 0
    Expected: Command exits 0 because excluded paths are absent from diff.
    Evidence: .omo/evidence/f1-scope-exclusions.txt
  ```
  **Commit**: NO | Message: `chore(kg): verify plan compliance` | Files: `.omo/evidence/f1-*`

- [x] F2. Code Quality Review — unspecified-high

  **What to do**: Review Rust module boundaries, error types, async/timeout handling, config serialization, test determinism, and dependency additions. Confirm code follows existing repo conventions and has no AI-slop duplication.
  **Must NOT do**: Do not expand feature scope or add UI.
  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: cross-module code quality review.
  - Skills: `[]` - no special skill required.
  - Omitted: `security-research` - not a full security audit.
  **Parallelization**: Can Parallel: YES | Final Wave | Blocks: completion | Blocked By: 1-9
  **References**:
  - New module: `frontend/src-tauri/src/knowledge_graph/`.
  - Existing patterns: `frontend/src-tauri/src/summary/commands.rs`, `frontend/src-tauri/src/api/api.rs`, `frontend/src-tauri/src/database/repositories/setting.rs`.
  **Acceptance Criteria**:
  - [ ] `cd frontend/src-tauri && cargo test knowledge_graph` passes.
  - [ ] `cd frontend/src-tauri && cargo test` passes or pre-existing unrelated failures are documented.
  - [ ] Review report approves module boundaries and error handling.
  **QA Scenarios**:
  ```
  Scenario: Rust KG tests pass
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test knowledge_graph
    Expected: Command exits 0.
    Evidence: .omo/evidence/f2-cargo-kg.txt

  Scenario: Full Rust test suite assessed
    Tool: Bash
    Steps: cd frontend/src-tauri && cargo test
    Expected: Command exits 0, or report lists exact pre-existing unrelated failures.
    Evidence: .omo/evidence/f2-cargo-full.txt
  ```
  **Commit**: NO | Message: `chore(kg): verify code quality` | Files: `.omo/evidence/f2-*`

- [x] F3. Real Manual QA — unspecified-high

  **What to do**: Run agent-executed QA commands that simulate real usage without human clicks: existing frontend tests, compose config render, optional Docker health if available, and command/fixture tests proving recorded-meeting ingestion behavior.
  **Must NOT do**: Do not require Playwright or manual UI interaction.
  **Recommended Agent Profile**:
  - Category: `unspecified-high` - Reason: end-to-end command QA across frontend, Rust, and compose config.
  - Skills: `[]` - no special skill required.
  - Omitted: `visual-engineering` - no screenshot/UI work in Week 1.
  **Parallelization**: Can Parallel: YES | Final Wave | Blocks: completion | Blocked By: 1-9
  **References**:
  - Test infra: `frontend/tests/lib/`, `frontend/package.json`, `frontend/src-tauri/Cargo.toml`.
  - Compose stack: `docker-compose.kg.yml`, `kg.env.example`.
  **Acceptance Criteria**:
  - [ ] `cd frontend && bun test tests/lib` passes.
  - [ ] `docker compose -f docker-compose.kg.yml --env-file kg.env.example config` passes.
  - [ ] Optional Docker startup either reaches healthy services or records unavailable Docker/dependency reason.
  **QA Scenarios**:
  ```
  Scenario: Existing frontend unit tests still pass
    Tool: Bash
    Steps: cd frontend && bun test tests/lib
    Expected: Command exits 0.
    Evidence: .omo/evidence/f3-bun-tests.txt

  Scenario: Local KG compose config is valid
    Tool: Bash
    Steps: docker compose -f docker-compose.kg.yml --env-file kg.env.example config
    Expected: Command exits 0 and includes rustfs, neo4j, lightrag.
    Evidence: .omo/evidence/f3-compose-config.txt
  ```
  **Commit**: NO | Message: `chore(kg): run qa verification` | Files: `.omo/evidence/f3-*`

- [x] F4. Scope Fidelity Check — deep

  **What to do**: Independently verify the implementation matches the user’s chosen Week 1 foundation and does not drift into later roadmap items. Inspect git diff, new commands, changed modules, and docs for automatic indexing, UI, Insight Engine, Helm, or Obsidian work.
  **Must NOT do**: Do not approve if implementation adds live recording hooks or graph UI without explicit user approval.
  **Recommended Agent Profile**:
  - Category: `deep` - Reason: adversarial scope review across product intent and implementation diff.
  - Skills: `[]` - no special skill required.
  - Omitted: `visual-engineering` - no design QA needed.
  **Parallelization**: Can Parallel: YES | Final Wave | Blocks: completion | Blocked By: 1-9
  **References**:
  - Vault decisions: `/Users/hermes/workspace/mind/Ideas/Meetily-Knowledge-Graph-Interface.md:399-426`.
  - Scope exclusions: `.omo/plans/meetingily-knowledge-graph-foundation.md` Must NOT Have section.
  **Acceptance Criteria**:
  - [ ] Report confirms Week 1 foundation only.
  - [ ] Report confirms KG default remains `None` and ingestion requires explicit command/profile.
  - [ ] Report confirms no archived backend, graph UI, Insight Engine, Helm, or Obsidian sync changes.
  **QA Scenarios**:
  ```
  Scenario: Diff stays within Week 1 scope
    Tool: Bash
    Steps: git diff --name-only
    Expected: Changed files are limited to KG Rust module, Tauri registration, database setup/manager changes for the ingestion ledger, Cargo files if needed, compose/env templates, and allowed docs/evidence.
    Evidence: .omo/evidence/f4-diff-scope.txt

  Scenario: No later-roadmap terms introduced in source changes
    Tool: Bash
    Steps: git diff -- . ':(exclude).omo/**' | grep -E "Insight Engine|knowledge gap|action item|decision capture|relationship synthesis|Helm|Obsidian"; test $? -ne 0
    Expected: Command exits 0 because later-roadmap feature terms are absent from implementation/source diff after excluding `.omo/**` planning/evidence files.
    Evidence: .omo/evidence/f4-roadmap-terms.txt
  ```
  **Commit**: NO | Message: `chore(kg): verify scope fidelity` | Files: `.omo/evidence/f4-*`

## Commit Strategy
- Do not commit unless the user explicitly asks the executor to commit.
- If commits are requested later, use one final commit after all verification passes: `feat(kg): add lightrag ingestion foundation`.
- Commit only intended files listed in task outputs; never include `.env` files or `.omo/evidence/` unless explicitly requested.

## Success Criteria
- Week 1 foundation exists entirely in the supported Tauri/Rust app.
- KG config is disabled by default and requires explicit profile selection for ingestion.
- LightRAG client targets verified v1.5+ API endpoints and auth behavior.
- Recorded-meeting transcript chunks can be sent through mock provider tests and, when Docker is available, validated against local compose stack health.
- Provider failures never break recording/transcription persistence.
- Scope stays foundation-only: no graph UI, Insight Engine, action items, decision capture, remote Helm, or Obsidian sync.
