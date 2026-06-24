import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import type { KgInvokeRegistry } from '../fixtures/kg-mock-invoke';
import {
  KgMockScenario,
  buildScenarioRegistry,
  createMockInvoke,
} from '../fixtures/kg-mock-invoke';
import {
  FIXTURE_HEALTH_HEALTHY,
  FIXTURE_INGESTION_SUCCESS,
  FIXTURE_PROFILE_KINDS,
  FIXTURE_QUERY_EMPTY,
  FIXTURE_QUERY_ERROR,
  FIXTURE_QUERY_MODES,
  FIXTURE_QUERY_SUCCESS,
  FIXTURE_SETTINGS_DEFAULT,
  FIXTURE_SETTINGS_WITH_ACTIVE,
} from '../fixtures/kg-fixtures';
import type {
  IngestionSummary,
  KnowledgeGraphHealth,
  KnowledgeGraphQueryResponse,
  KnowledgeGraphSettings,
  QueryMode,
} from '../../src/types/knowledgeGraph';
import {
  DEFAULT_QUERY_MODE,
  DEFAULT_TOP_K,
} from '../../src/types/knowledgeGraph';

// ── Mock invoke setup ────────────────────────────────────────────────

let registry: KgInvokeRegistry;
let mockInvoke: ReturnType<typeof createMockInvoke>;

// Patch the module before importing the service.
// `mock.module` replaces the export so `invoke` calls hit our mock.
mock.module('@tauri-apps/api/core', () => ({
  invoke: (...args: Parameters<typeof mockInvoke>) => mockInvoke(...args),
}));

// Import after mock is set up.
const { KnowledgeGraphService } = await import(
  '../../src/services/knowledgeGraphService'
);

// ── Tests ────────────────────────────────────────────────────────────

describe('KnowledgeGraphService', () => {
  beforeEach(() => {
    registry = buildScenarioRegistry(KgMockScenario.HEALTHY);
    mockInvoke = createMockInvoke(registry);
  });

  afterEach(() => {
    registry.clear();
  });

  // ── getSettings ──────────────────────────────────────────────────

  test('getSettings calls api_get_knowledge_graph_settings with no args', async () => {
    const service = new KnowledgeGraphService();
    const result = await service.getSettings();

    expect(result).toEqual(FIXTURE_SETTINGS_DEFAULT);
    const calls = registry.getCallsFor('api_get_knowledge_graph_settings');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({});
  });

  test('getSettings returns typed KnowledgeGraphSettings', async () => {
    const service = new KnowledgeGraphService();
    const result = await service.getSettings();

    expect(result.profiles).toBeArray();
    expect(result.profiles.length).toBeGreaterThan(0);
    expect(result.active_profile).toBeDefined();
  });

  // ── saveSettings ─────────────────────────────────────────────────

  test('saveSettings calls api_save_knowledge_graph_settings with settings arg', async () => {
    const service = new KnowledgeGraphService();
    const result = await service.saveSettings(FIXTURE_SETTINGS_WITH_ACTIVE);

    expect(result).toEqual(FIXTURE_SETTINGS_DEFAULT);
    const calls = registry.getCallsFor('api_save_knowledge_graph_settings');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({ settings: FIXTURE_SETTINGS_WITH_ACTIVE });
  });

  // ── testProfile ───────────────────────────────────────────────────

  test('testProfile calls api_test_knowledge_graph_profile with profileId', async () => {
    const service = new KnowledgeGraphService();
    const result = await service.testProfile('local-1');

    expect(result).toEqual(FIXTURE_HEALTH_HEALTHY);
    const calls = registry.getCallsFor('api_test_knowledge_graph_profile');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({ profileId: 'local-1' });
  });

  test('testProfile rejects for unknown profile', async () => {
    const service = new KnowledgeGraphService();
    await expect(service.testProfile('nonexistent')).rejects.toThrow(
      "Profile 'nonexistent' not found"
    );
  });

  // ── ingestMeeting ─────────────────────────────────────────────────

  test('ingestMeeting calls api_ingest_meeting_to_knowledge_graph with meetingId and profileId', async () => {
    const service = new KnowledgeGraphService();
    const result = await service.ingestMeeting('meeting-001', 'local-1');

    expect(result.submitted_count).toBe(12);
    expect(result.failed_count).toBe(0);
    expect(result.meeting_id).toBe('meeting-001');
    expect(result.profile_id).toBe('local-1');

    const calls = registry.getCallsFor('api_ingest_meeting_to_knowledge_graph');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({ meetingId: 'meeting-001', profileId: 'local-1' });
  });

  // ── queryKnowledgeGraph ───────────────────────────────────────────

  test('queryKnowledgeGraph calls api_query_knowledge_graph with all args', async () => {
    const service = new KnowledgeGraphService();
    const result = await service.queryKnowledgeGraph('local-1', 'action items', 'local', 10);

    expect(result).toEqual(FIXTURE_QUERY_SUCCESS);
    const calls = registry.getCallsFor('api_query_knowledge_graph');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({
      profileId: 'local-1',
      query: 'action items',
      mode: 'local',
      topK: 10,
    });
  });

  test('queryKnowledgeGraph uses default mode and topK when omitted', async () => {
    const service = new KnowledgeGraphService();
    await service.queryKnowledgeGraph('local-1', 'action items');

    const calls = registry.getCallsFor('api_query_knowledge_graph');
    expect(calls[0].args).toEqual({
      profileId: 'local-1',
      query: 'action items',
      mode: DEFAULT_QUERY_MODE,
      topK: DEFAULT_TOP_K,
    });
  });
});

// ── Type Serialization Tests ─────────────────────────────────────────

describe('KG type serialization', () => {
  test('KnowledgeGraphSelection "none" serializes as string', () => {
    const settings: KnowledgeGraphSettings = FIXTURE_SETTINGS_DEFAULT;
    expect(settings.active_profile).toBe('none');
  });

  test('KnowledgeGraphSelection profile serializes as tagged object', () => {
    const settings: KnowledgeGraphSettings = FIXTURE_SETTINGS_WITH_ACTIVE;
    expect(settings.active_profile).toEqual({ profile: 'remote-1' });
  });

  test('QueryMode all variants are valid string literals', () => {
    for (const mode of FIXTURE_QUERY_MODES) {
      expect(typeof mode).toBe('string');
    }
    expect(FIXTURE_QUERY_MODES).toEqual([
      'local',
      'global',
      'hybrid',
      'naive',
      'mix',
      'bypass',
    ]);
  });

  test('ProfileKind all variants are valid string literals', () => {
    expect(FIXTURE_PROFILE_KINDS).toEqual(['local', 'remote']);
  });

  test('IngestionSummary has correct shape', () => {
    const summary: IngestionSummary = FIXTURE_INGESTION_SUCCESS;
    expect(summary.submitted_count).toBeNumber();
    expect(summary.already_submitted_count).toBeNumber();
    expect(summary.failed_count).toBeNumber();
    expect(summary.failed_chunks).toBeArray();
    expect(summary.meeting_id).toBeString();
    expect(summary.profile_id).toBeString();
  });

  test('KnowledgeGraphQueryResponse success has nodes and edges', () => {
    const resp: KnowledgeGraphQueryResponse = FIXTURE_QUERY_SUCCESS;
    expect(resp.answer).toBeString();
    expect(resp.nodes.length).toBeGreaterThan(0);
    expect(resp.edges.length).toBeGreaterThan(0);
    expect(resp.nodes[0].id).toBeString();
    expect(resp.nodes[0].label).toBeString();
    expect(resp.edges[0].source).toBeString();
    expect(resp.edges[0].target).toBeString();
    expect(resp.edges[0].relation).toBeString();
  });

  test('KnowledgeGraphQueryResponse empty has no nodes or edges', () => {
    const resp: KnowledgeGraphQueryResponse = FIXTURE_QUERY_EMPTY;
    expect(resp.answer).toBeUndefined();
    expect(resp.nodes).toEqual([]);
    expect(resp.edges).toEqual([]);
  });

  test('KnowledgeGraphHealth has healthy boolean and optional version', () => {
    const health: KnowledgeGraphHealth = FIXTURE_HEALTH_HEALTHY;
    expect(health.healthy).toBe(true);
    expect(health.version).toBe('2.0.0');
  });

  test('DEFAULT_QUERY_MODE is hybrid', () => {
    expect(DEFAULT_QUERY_MODE).toBe('hybrid');
  });

  test('DEFAULT_TOP_K is 5', () => {
    expect(DEFAULT_TOP_K).toBe(5);
  });
});

// ── Mock Registry Tests ───────────────────────────────────────────────

describe('KgInvokeRegistry', () => {
  test('rejects unregistered commands', async () => {
    const reg = buildScenarioRegistry(KgMockScenario.QUERY_SUCCESS);
    const invoke = createMockInvoke(reg);

    await expect(invoke('unknown_command')).rejects.toThrow(
      'No mock handler registered for command: unknown_command'
    );
  });

  test('records call log for assertions', async () => {
    const reg = buildScenarioRegistry(KgMockScenario.HEALTHY);
    const invoke = createMockInvoke(reg);

    await invoke('api_get_knowledge_graph_settings');
    await invoke('api_test_knowledge_graph_profile', { profileId: 'local-1' });

    const calls = reg.getCalls();
    expect(calls.length).toBe(2);
    expect(calls[0].command).toBe('api_get_knowledge_graph_settings');
    expect(calls[1].command).toBe('api_test_knowledge_graph_profile');
  });

  test('scenario NO_PROFILE rejects ingestion and query', async () => {
    const reg = buildScenarioRegistry(KgMockScenario.NO_PROFILE);
    const invoke = createMockInvoke(reg);

    await expect(
      invoke('api_ingest_meeting_to_knowledge_graph', {
        meetingId: 'meeting-001',
        profileId: 'none',
      })
    ).rejects.toThrow("Invalid profile_id: 'none'");

    await expect(
      invoke('api_query_knowledge_graph', {
        profileId: 'none',
        query: 'test',
      })
    ).rejects.toThrow("Invalid profile_id: 'none'");
  });

  test('scenario UNHEALTHY returns unhealthy health', async () => {
    const reg = buildScenarioRegistry(KgMockScenario.UNHEALTHY);
    const invoke = createMockInvoke(reg);

    const result = await invoke('api_test_knowledge_graph_profile', {
      profileId: 'local-1',
    });
    expect(result).toEqual({ healthy: false, version: undefined });
  });

  test('scenario INGEST_PARTIAL_FAILURE returns partial summary', async () => {
    const reg = buildScenarioRegistry(KgMockScenario.INGEST_PARTIAL_FAILURE);
    const invoke = createMockInvoke(reg);

    const result = (await invoke('api_ingest_meeting_to_knowledge_graph', {
      meetingId: 'meeting-001',
      profileId: 'remote-1',
    })) as IngestionSummary;

    expect(result.submitted_count).toBe(8);
    expect(result.already_submitted_count).toBe(2);
    expect(result.failed_count).toBe(2);
    expect(result.failed_chunks.length).toBe(2);
  });

  test('scenario QUERY_EMPTY returns empty response', async () => {
    const reg = buildScenarioRegistry(KgMockScenario.QUERY_EMPTY);
    const invoke = createMockInvoke(reg);

    const result = (await invoke('api_query_knowledge_graph', {
      profileId: 'local-1',
      query: 'nonexistent topic',
    })) as KnowledgeGraphQueryResponse;

    expect(result.answer).toBeUndefined();
    expect(result.nodes).toEqual([]);
    expect(result.edges).toEqual([]);
  });

  test('scenario QUERY_ERROR rejects with error', async () => {
    const reg = buildScenarioRegistry(KgMockScenario.QUERY_ERROR);
    const invoke = createMockInvoke(reg);

    await expect(
      invoke('api_query_knowledge_graph', {
        profileId: 'local-1',
        query: 'error trigger',
      })
    ).rejects.toThrow('Query failed: LightRAG returned HTTP 500');
  });
});