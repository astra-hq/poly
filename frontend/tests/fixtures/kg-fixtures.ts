/**
 * Knowledge Graph Test Fixtures
 *
 * Deterministic data for browser QA and unit tests.
 * Every value is hardcoded — no random generation, no timestamps.
 *
 * Organized by scenario:
 * 1. Profiles (local, remote, selected-profile, no-profile)
 * 2. Settings (with/without active profile)
 * 3. Health (healthy, unhealthy, error)
 * 4. Meetings (saved, selected-profile, no-profile)
 * 5. Ingestion (success, partial-failure, full-failure)
 * 6. Query (success, empty, error)
 * 7. Recording selector paths (mic, system, meeting name)
 */

import type {
  EmbeddingConfig,
  IngestionSummary,
  KnowledgeGraphHealth,
  KnowledgeGraphProfile,
  KnowledgeGraphQueryResponse,
  KnowledgeGraphSettings,
  KnowledgeGraphSelection,
  ProfileKind,
  QueryMode,
} from '../../src/types/knowledgeGraph';

// ── Embedding Configs ────────────────────────────────────────────────

export const FIXTURE_EMBEDDING_MLX: EmbeddingConfig = {
  provider: 'mlx',
  model: 'BAAI/bge-m3',
  dimensions: 1024,
};

export const FIXTURE_EMBEDDING_OPENAI: EmbeddingConfig = {
  provider: 'openai',
  model: 'text-embedding-3-large',
  dimensions: 3072,
};

// ── Profiles ────────────────────────────────────────────────────────

export const FIXTURE_PROFILE_LOCAL: KnowledgeGraphProfile = {
  id: 'local-1',
  name: 'Local Dev',
  kind: 'local',
  embedding: FIXTURE_EMBEDDING_MLX,
  lightrag_url: 'http://localhost:9621',
  api_key: undefined,
  notes: 'Local development LightRAG instance',
};

export const FIXTURE_PROFILE_REMOTE: KnowledgeGraphProfile = {
  id: 'remote-1',
  name: 'Production KG',
  kind: 'remote',
  embedding: FIXTURE_EMBEDDING_OPENAI,
  lightrag_url: 'https://kg.example.com',
  api_key: 'kg-secret-key-123',
  notes: 'Production cluster with OpenAI embeddings',
};

export const FIXTURE_PROFILE_SELECTED: KnowledgeGraphProfile = {
  id: 'selected-1',
  name: 'Selected Profile',
  kind: 'local',
  embedding: FIXTURE_EMBEDDING_MLX,
  lightrag_url: 'http://localhost:9621',
  api_key: 'dummy-key',
  notes: undefined,
};

export const FIXTURE_PROFILE_NO_PROFILE: KnowledgeGraphProfile = {
  id: 'none',
  name: 'No Profile',
  kind: 'local',
  embedding: FIXTURE_EMBEDDING_MLX,
  lightrag_url: 'http://localhost:9621',
  api_key: undefined,
  notes: undefined,
};

// ── Settings ────────────────────────────────────────────────────────

export const FIXTURE_SETTINGS_DEFAULT: KnowledgeGraphSettings = {
  profiles: [FIXTURE_PROFILE_LOCAL],
  active_profile: 'none',
};

export const FIXTURE_SETTINGS_WITH_ACTIVE: KnowledgeGraphSettings = {
  profiles: [FIXTURE_PROFILE_LOCAL, FIXTURE_PROFILE_REMOTE],
  active_profile: { profile: 'remote-1' },
};

export const FIXTURE_SETTINGS_SELECTED_PROFILE: KnowledgeGraphSettings = {
  profiles: [FIXTURE_PROFILE_SELECTED],
  active_profile: { profile: 'selected-1' },
};

export const FIXTURE_SETTINGS_NO_PROFILE: KnowledgeGraphSettings = {
  profiles: [FIXTURE_PROFILE_NO_PROFILE],
  active_profile: 'none',
};

// ── Health ────────────────────────────────────────────────────────────

export const FIXTURE_HEALTH_HEALTHY: KnowledgeGraphHealth = {
  healthy: true,
  version: '2.0.0',
};

export const FIXTURE_HEALTH_UNHEALTHY: KnowledgeGraphHealth = {
  healthy: false,
  version: undefined,
};

export const FIXTURE_HEALTH_ERROR: KnowledgeGraphHealth = {
  healthy: false,
  version: undefined,
};

// ── Meetings ─────────────────────────────────────────────────────────

export interface FixtureMeeting {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  folder_path: string | null;
}

export const FIXTURE_MEETING_SAVED: FixtureMeeting = {
  id: 'meeting-001',
  title: 'Team Standup 2024-01-15',
  created_at: '2024-01-15T09:00:00Z',
  updated_at: '2024-01-15T09:30:00Z',
  folder_path: '/Users/test/meetings/standup-001',
};

export const FIXTURE_MEETING_SELECTED_PROFILE: FixtureMeeting = {
  id: 'meeting-002',
  title: 'Product Review with KG',
  created_at: '2024-01-16T14:00:00Z',
  updated_at: '2024-01-16T15:00:00Z',
  folder_path: '/Users/test/meetings/review-002',
};

export const FIXTURE_MEETING_NO_PROFILE: FixtureMeeting = {
  id: 'meeting-003',
  title: 'Quick Sync (no KG)',
  created_at: '2024-01-17T10:00:00Z',
  updated_at: '2024-01-17T10:15:00Z',
  folder_path: null,
};

// ── Ingestion Summaries ─────────────────────────────────────────────

export const FIXTURE_INGESTION_SUCCESS: IngestionSummary = {
  submitted_count: 12,
  already_submitted_count: 0,
  failed_count: 0,
  failed_chunks: [],
  meeting_id: 'meeting-001',
  profile_id: 'local-1',
};

export const FIXTURE_INGESTION_PARTIAL_FAILURE: IngestionSummary = {
  submitted_count: 8,
  already_submitted_count: 2,
  failed_count: 2,
  failed_chunks: [
    { sequence: 9, error: 'Connection timeout' },
    { sequence: 10, error: 'HTTP 500 from LightRAG' },
  ],
  meeting_id: 'meeting-001',
  profile_id: 'remote-1',
};

export const FIXTURE_INGESTION_FULL_FAILURE: IngestionSummary = {
  submitted_count: 0,
  already_submitted_count: 0,
  failed_count: 5,
  failed_chunks: [
    { sequence: 0, error: 'Connection refused' },
    { sequence: 1, error: 'Connection refused' },
    { sequence: 2, error: 'Connection refused' },
    { sequence: 3, error: 'Connection refused' },
    { sequence: 4, error: 'Connection refused' },
  ],
  meeting_id: 'meeting-003',
  profile_id: 'local-1',
};

export const FIXTURE_INGESTION_ALL_ALREADY_SUBMITTED: IngestionSummary = {
  submitted_count: 0,
  already_submitted_count: 12,
  failed_count: 0,
  failed_chunks: [],
  meeting_id: 'meeting-001',
  profile_id: 'local-1',
};

// ── Query Responses ─────────────────────────────────────────────────

export const FIXTURE_QUERY_SUCCESS: KnowledgeGraphQueryResponse = {
  answer: 'The team decided to ship the feature on Friday and assign two engineers to the task.',
  nodes: [
    {
      id: 'node-1',
      label: 'Person',
      properties: { name: 'Alice', role: 'Engineer' },
    },
    {
      id: 'node-2',
      label: 'Task',
      properties: { name: 'Ship Feature X', deadline: 'Friday' },
    },
  ],
  edges: [
    {
      source: 'node-1',
      target: 'node-2',
      relation: 'assigned_to',
      properties: { hours: '20' },
    },
  ],
};

export const FIXTURE_QUERY_EMPTY: KnowledgeGraphQueryResponse = {
  answer: undefined,
  nodes: [],
  edges: [],
};

export const FIXTURE_QUERY_ERROR: KnowledgeGraphQueryResponse = {
  answer: undefined,
  nodes: [],
  edges: [],
};

// ── Query Modes ──────────────────────────────────────────────────────

export const FIXTURE_QUERY_MODES: QueryMode[] = [
  'local',
  'global',
  'hybrid',
  'naive',
  'mix',
  'bypass',
];

// ── Recording Selector Paths ────────────────────────────────────────

export interface FixtureRecordingSelector {
  mic_device_name: string | null;
  system_device_name: string | null;
  meeting_name: string | null;
}

export const FIXTURE_RECORDING_SELECTOR_FULL: FixtureRecordingSelector = {
  mic_device_name: 'Built-in Microphone',
  system_device_name: 'BlackHole 2ch',
  meeting_name: 'Team Standup',
};

export const FIXTURE_RECORDING_SELECTOR_MIC_ONLY: FixtureRecordingSelector = {
  mic_device_name: 'Built-in Microphone',
  system_device_name: null,
  meeting_name: 'Quick Note',
};

export const FIXTURE_RECORDING_SELECTOR_SYSTEM_ONLY: FixtureRecordingSelector = {
  mic_device_name: null,
  system_device_name: 'BlackHole 2ch',
  meeting_name: 'System Audio Capture',
};

export const FIXTURE_RECORDING_SELECTOR_EMPTY: FixtureRecordingSelector = {
  mic_device_name: null,
  system_device_name: null,
  meeting_name: null,
};

// ── Selection Helpers ────────────────────────────────────────────────

export const FIXTURE_SELECTION_NONE: KnowledgeGraphSelection = 'none';
export const FIXTURE_SELECTION_LOCAL: KnowledgeGraphSelection = { profile: 'local-1' };
export const FIXTURE_SELECTION_REMOTE: KnowledgeGraphSelection = { profile: 'remote-1' };

// ── Profile Kind Helpers ──────────────────────────────────────────────

export const FIXTURE_PROFILE_KINDS: ProfileKind[] = ['local', 'remote'];