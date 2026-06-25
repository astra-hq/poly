/**
 * Knowledge Graph Types
 *
 * TypeScript mirrors of the Rust serde types in
 * `src-tauri/src/knowledge_graph/{types,config,service}.rs`.
 *
 * Serialization conventions:
 * - Rust enums with `#[serde(rename_all = "snake_case")]` map to
 *   string-literal unions or tagged objects.
 * - Optional Rust fields (`Option<T>` with `skip_serializing_if`) map to
 *   `T | undefined` (not `T | null`) to match JSON omission.
 */

// ── Query Mode (types.rs) ────────────────────────────────────────────

/**
 * Query mode for knowledge graph retrieval.
 * Rust: `QueryMode` enum with `#[serde(rename_all = "snake_case")]`.
 */
export type QueryMode =
  | 'local'
  | 'global'
  | 'hybrid'
  | 'naive'
  | 'mix'
  | 'bypass';

export const DEFAULT_QUERY_MODE: QueryMode = 'hybrid';
export const DEFAULT_TOP_K = 5;

// ── Profile Kind (config.rs) ──────────────────────────────────────────

/**
 * Whether a profile targets a local or remote LightRAG instance.
 * Rust: `ProfileKind` enum with `#[serde(rename_all = "snake_case")]`.
 */
export type ProfileKind = 'local' | 'remote';

// ── Embedding Config (config.rs) ─────────────────────────────────────

export interface EmbeddingConfig {
  provider: string;
  model: string;
  dimensions: number;
}

export const DEFAULT_EMBEDDING_CONFIG: EmbeddingConfig = {
  provider: 'mlx',
  model: 'BAAI/bge-m3',
  dimensions: 1024,
};

// ── Knowledge Graph Profile (config.rs) ──────────────────────────────

export interface KnowledgeGraphProfile {
  id: string;
  name: string;
  kind: ProfileKind;
  embedding: EmbeddingConfig;
  lightrag_url: string;
  api_key?: string;
  notes?: string;
}

export const DEFAULT_KG_PROFILE: KnowledgeGraphProfile = {
  id: 'default',
  name: 'default',
  kind: 'local',
  embedding: DEFAULT_EMBEDDING_CONFIG,
  lightrag_url: 'http://localhost:9621',
};

// ── Knowledge Graph Selection (config.rs) ────────────────────────────

/**
 * Which profile is currently selected.
 * Rust: `KnowledgeGraphSelection` enum with `#[serde(rename_all = "snake_case")]`.
 * - `None` serializes as `"none"`
 * - `Profile("id")` serializes as `{ "profile": "id" }`
 */
export type KnowledgeGraphSelection =
  | 'none'
  | { profile: string };

// ── Knowledge Graph Settings (config.rs) ─────────────────────────────

export interface KnowledgeGraphSettings {
  profiles: KnowledgeGraphProfile[];
  active_profile: KnowledgeGraphSelection;
}

export const DEFAULT_KG_SETTINGS: KnowledgeGraphSettings = {
  profiles: [],
  active_profile: 'none',
};

// ── Health (types.rs) ────────────────────────────────────────────────

export interface KnowledgeGraphHealth {
  healthy: boolean;
  version?: string;
}

// ── Graph Nodes / Edges (types.rs) ────────────────────────────────────

export interface KnowledgeGraphNode {
  id: string;
  label: string;
  properties?: Record<string, string>;
}

export interface KnowledgeGraphEdge {
  source: string;
  target: string;
  relation: string;
  properties?: Record<string, string>;
}

// ── Query Response (types.rs) ────────────────────────────────────────

export interface KnowledgeGraphQueryResponse {
  answer?: string;
  nodes: KnowledgeGraphNode[];
  edges: KnowledgeGraphEdge[];
}

// ── Pipeline Status (types.rs) ───────────────────────────────────────

export interface KnowledgeGraphPipelineStatus {
  pending_documents: number;
  indexing_documents: number;
  failed_documents: number;
}

// ── Track Status Document (types.rs) ─────────────────────────────────

export interface TrackStatusDocument {
  id: string;
  content_summary: string;
  content_length: number;
  status: string;
  created_at: string;
  updated_at: string;
  track_id?: string;
  chunks_count?: number;
  error_msg?: string;
  metadata?: Record<string, string>;
  file_path: string;
}

// ── Track Status (types.rs) ──────────────────────────────────────────

export interface KnowledgeGraphTrackStatus {
  track_id: string;
  documents: TrackStatusDocument[];
  total_count: number;
  status_summary?: Record<string, number>;
}

// ── Job State (types.rs) ─────────────────────────────────────────────

/**
 * Rust: `KnowledgeGraphJobState` enum with `#[serde(rename_all = "snake_case")]`.
 */
export type KnowledgeGraphJobState =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed';

// ── Ingestion Summary (service.rs) ───────────────────────────────────

export interface FailedChunk {
  sequence: number;
  error: string;
}

export interface IngestionSummary {
  submitted_count: number;
  already_submitted_count: number;
  failed_count: number;
  failed_chunks: FailedChunk[];
  meeting_id: string;
  profile_id: string;
}

// ── Meeting Knowledge Graph Selection (selection_commands.rs) ──────────

export interface MeetingKnowledgeGraphSelection {
  meeting_id: string;
  /** `null` means explicit "none" selection. */
  profile_id: string | null;
  meeting_type: string | null;
  routing_reason: string | null;
}

export type SummaryDocumentState = 'pending' | 'ingested' | 'failed' | 'deleted';

export interface SummaryDocumentStatus {
  state: SummaryDocumentState;
  file_source?: string;
  error?: string;
  updated_at?: string;
  track_id?: string;
  document_id?: string;
}

// ── Meeting Knowledge Graph Status (service.rs) ──────────────────────

export interface MeetingKnowledgeGraphStatus {
  meeting_id: string;
  selected_profile_id: string | null;
  is_indexing_available: boolean;
  submitted_count: number;
  failed_count: number;
  pending_count: number;
  total_chunks: number;
  last_errors: string[];
  summary_document?: SummaryDocumentStatus;
  pipeline_status?: KnowledgeGraphPipelineStatus;
  lightrag_url?: string;
}

// ── Summary Ingest Result (commands.rs) ──────────────────────────────

export interface SummaryIngestResult {
  meeting_id: string;
  profile_id: string | null;
  ingested: boolean;
  error: string | null;
}

// ── Status States (for UI consumption) ────────────────────────────────

/**
 * High-level ingestion status for UI state machines.
 * Derived from `KnowledgeGraphJobState` but simplified for component use.
 */
export type IngestionStatusState =
  | 'idle'
  | 'ingesting'
  | 'completed'
  | 'failed';

/**
 * Query status for UI state machines.
 */
export type QueryStatusState =
  | 'idle'
  | 'querying'
  | 'success'
  | 'empty'
  | 'error';