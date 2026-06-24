/**
 * Knowledge Graph Mock Invoke Harness
 *
 * Provides a deterministic mock of Tauri's `invoke` function for
 * browser QA (Playwright) and unit tests.  Instead of calling the
 * Rust backend, the mock looks up the command name in a handler map
 * and returns the registered fixture data.
 *
 * Usage in tests:
 *
 *   import { createKgMockInvoke, KgMockScenario } from '../fixtures/kg-mock-invoke';
 *
 *   const mock = createKgMockInvoke(KgMockScenario.HEALTHY);
 *   // Patch the module so `invoke` calls hit the mock:
 *   mock.install();
 *   // ... run test code that calls knowledgeGraphService methods ...
 *   mock.uninstall();
 *
 * Or use the lower-level `KgInvokeRegistry` directly to register
 * custom handlers.
 */

import type {
  IngestionSummary,
  KnowledgeGraphHealth,
  KnowledgeGraphQueryResponse,
  KnowledgeGraphSettings,
} from '../../src/types/knowledgeGraph';
import {
  FIXTURE_HEALTH_HEALTHY,
  FIXTURE_HEALTH_UNHEALTHY,
  FIXTURE_INGESTION_ALL_ALREADY_SUBMITTED,
  FIXTURE_INGESTION_FULL_FAILURE,
  FIXTURE_INGESTION_PARTIAL_FAILURE,
  FIXTURE_INGESTION_SUCCESS,
  FIXTURE_MEETING_NO_PROFILE,
  FIXTURE_MEETING_SAVED,
  FIXTURE_MEETING_SELECTED_PROFILE,
  FIXTURE_QUERY_EMPTY,
  FIXTURE_QUERY_ERROR,
  FIXTURE_QUERY_SUCCESS,
  FIXTURE_RECORDING_SELECTOR_EMPTY,
  FIXTURE_RECORDING_SELECTOR_FULL,
  FIXTURE_RECORDING_SELECTOR_MIC_ONLY,
  FIXTURE_RECORDING_SELECTOR_SYSTEM_ONLY,
  FIXTURE_SETTINGS_DEFAULT,
  FIXTURE_SETTINGS_NO_PROFILE,
  FIXTURE_SETTINGS_SELECTED_PROFILE,
  FIXTURE_SETTINGS_WITH_ACTIVE,
} from './kg-fixtures';

// ── Types ────────────────────────────────────────────────────────────

/** A handler function for a mocked Tauri command. */
export type KgInvokeHandler = (args: Record<string, unknown>) => unknown;

/** Registry mapping command names to handler functions. */
export class KgInvokeRegistry {
  private handlers = new Map<string, KgInvokeHandler>();
  private callLog: { command: string; args: Record<string, unknown> }[] = [];

  /** Register a handler for a command. */
  register(command: string, handler: KgInvokeHandler): void {
    this.handlers.set(command, handler);
  }

  /** Unregister a command. */
  unregister(command: string): void {
    this.handlers.delete(command);
  }

  /** Clear all handlers and call log. */
  clear(): void {
    this.handlers.clear();
    this.callLog = [];
  }

  /** Look up and invoke the handler for a command. */
  resolve(command: string, args: Record<string, unknown>): unknown {
    this.callLog.push({ command, args });
    const handler = this.handlers.get(command);
    if (!handler) {
      return Promise.reject(
        new Error(`No mock handler registered for command: ${command}`)
      );
    }
    const result = handler(args);
    return Promise.resolve(result);
  }

  /** Return the call log for assertions. */
  getCalls(): { command: string; args: Record<string, unknown> }[] {
    return [...this.callLog];
  }

  /** Get calls for a specific command. */
  getCallsFor(command: string): { command: string; args: Record<string, unknown> }[] {
    return this.callLog.filter((c) => c.command === command);
  }

  /** Reset call log without clearing handlers. */
  resetCallLog(): void {
    this.callLog = [];
  }
}

// ── Scenarios ────────────────────────────────────────────────────────

/**
 * Pre-built scenario identifiers for common QA flows.
 * Each scenario populates the registry with a coherent set of responses.
 */
export enum KgMockScenario {
  /** All commands succeed with healthy data. */
  HEALTHY = 'healthy',
  /** Settings exist with an active remote profile. */
  WITH_ACTIVE_PROFILE = 'with-active-profile',
  /** Settings exist but no profile is selected. */
  NO_PROFILE = 'no-profile',
  /** Selected profile is active. */
  SELECTED_PROFILE = 'selected-profile',
  /** Health check returns unhealthy. */
  UNHEALTHY = 'unhealthy',
  /** Ingestion succeeds with all chunks submitted. */
  INGEST_SUCCESS = 'ingest-success',
  /** Ingestion partially fails. */
  INGEST_PARTIAL_FAILURE = 'ingest-partial-failure',
  /** Ingestion fully fails. */
  INGEST_FULL_FAILURE = 'ingest-full-failure',
  /** Query returns results. */
  QUERY_SUCCESS = 'query-success',
  /** Query returns empty results. */
  QUERY_EMPTY = 'query-empty',
  /** Query errors. */
  QUERY_ERROR = 'query-error',
  /** All already submitted (idempotent re-ingest). */
  INGEST_ALL_ALREADY = 'ingest-all-already',
}

// ── Handler Factories ────────────────────────────────────────────────

/** Default handler: return a fixed value, ignoring args. */
function constHandler<T>(value: T): KgInvokeHandler {
  return () => value;
}

/** Handler that rejects with a fixed error message. */
function errorHandler(message: string): KgInvokeHandler {
  return () => {
    throw new Error(message);
  };
}

/** Handler for api_test_knowledge_graph_profile — returns health based on profileId. */
function healthHandler(
  healthyProfileIds: string[],
  unhealthyProfileIds: string[]
): KgInvokeHandler {
  return (args) => {
    const profileId = args['profileId'] as string;
    if (healthyProfileIds.includes(profileId)) {
      return FIXTURE_HEALTH_HEALTHY;
    }
    if (unhealthyProfileIds.includes(profileId)) {
      return FIXTURE_HEALTH_UNHEALTHY;
    }
    throw new Error(`Profile '${profileId}' not found in knowledge graph settings`);
  };
}

/** Handler for api_ingest_meeting_to_knowledge_graph — returns summary based on meetingId. */
function ingestHandler(
  successMeetingIds: string[],
  partialFailureMeetingIds: string[],
  fullFailureMeetingIds: string[],
  allAlreadyMeetingIds: string[]
): KgInvokeHandler {
  return (args) => {
    const meetingId = args['meetingId'] as string;
    const profileId = args['profileId'] as string;
    if (successMeetingIds.includes(meetingId)) {
      return { ...FIXTURE_INGESTION_SUCCESS, meeting_id: meetingId, profile_id: profileId };
    }
    if (partialFailureMeetingIds.includes(meetingId)) {
      return { ...FIXTURE_INGESTION_PARTIAL_FAILURE, meeting_id: meetingId, profile_id: profileId };
    }
    if (fullFailureMeetingIds.includes(meetingId)) {
      return { ...FIXTURE_INGESTION_FULL_FAILURE, meeting_id: meetingId, profile_id: profileId };
    }
    if (allAlreadyMeetingIds.includes(meetingId)) {
      return { ...FIXTURE_INGESTION_ALL_ALREADY_SUBMITTED, meeting_id: meetingId, profile_id: profileId };
    }
    throw new Error(`Meeting '${meetingId}' not found`);
  };
}

/** Handler for api_query_knowledge_graph — returns response based on query content. */
function queryHandler(
  successQueries: string[],
  emptyQueries: string[],
  errorQueries: string[]
): KgInvokeHandler {
  return (args) => {
    const query = args['query'] as string;
    if (errorQueries.some((q) => query.toLowerCase().includes(q.toLowerCase()))) {
      throw new Error('Query failed: LightRAG returned HTTP 500');
    }
    if (emptyQueries.some((q) => query.toLowerCase().includes(q.toLowerCase()))) {
      return FIXTURE_QUERY_EMPTY;
    }
    if (successQueries.some((q) => query.toLowerCase().includes(q.toLowerCase()))) {
      return FIXTURE_QUERY_SUCCESS;
    }
    // Default: return success
    return FIXTURE_QUERY_SUCCESS;
  };
}

// ── Scenario Builder ─────────────────────────────────────────────────

/**
 * Build a registry pre-populated for the given scenario.
 * Multiple scenarios can be combined by calling this function
 * multiple times and merging registries.
 */
export function buildScenarioRegistry(
  scenario: KgMockScenario
): KgInvokeRegistry {
  const registry = new KgInvokeRegistry();

  switch (scenario) {
    case KgMockScenario.HEALTHY:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_save_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_test_knowledge_graph_profile', healthHandler(['local-1'], []));
      registry.register('api_ingest_meeting_to_knowledge_graph', ingestHandler(['meeting-001'], [], [], []));
      registry.register('api_query_knowledge_graph', queryHandler(['action items'], [], []));
      break;

    case KgMockScenario.WITH_ACTIVE_PROFILE:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_WITH_ACTIVE));
      registry.register('api_save_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_WITH_ACTIVE));
      registry.register('api_test_knowledge_graph_profile', healthHandler(['remote-1'], ['local-1']));
      registry.register('api_ingest_meeting_to_knowledge_graph', ingestHandler(['meeting-001'], [], [], []));
      registry.register('api_query_knowledge_graph', queryHandler(['action items'], [], []));
      break;

    case KgMockScenario.NO_PROFILE:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_NO_PROFILE));
      registry.register('api_save_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_NO_PROFILE));
      registry.register('api_test_knowledge_graph_profile', healthHandler([], []));
      registry.register('api_ingest_meeting_to_knowledge_graph', errorHandler("Invalid profile_id: 'none'"));
      registry.register('api_query_knowledge_graph', errorHandler("Invalid profile_id: 'none'"));
      break;

    case KgMockScenario.SELECTED_PROFILE:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_SELECTED_PROFILE));
      registry.register('api_save_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_SELECTED_PROFILE));
      registry.register('api_test_knowledge_graph_profile', healthHandler(['selected-1'], []));
      registry.register('api_ingest_meeting_to_knowledge_graph', ingestHandler(['meeting-002'], [], [], []));
      registry.register('api_query_knowledge_graph', queryHandler(['action items'], [], []));
      break;

    case KgMockScenario.UNHEALTHY:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_save_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_test_knowledge_graph_profile', healthHandler([], ['local-1']));
      registry.register('api_ingest_meeting_to_knowledge_graph', errorHandler('Health check failed for profile local-1'));
      registry.register('api_query_knowledge_graph', errorHandler('Health check failed for profile local-1'));
      break;

    case KgMockScenario.INGEST_SUCCESS:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_ingest_meeting_to_knowledge_graph', ingestHandler(['meeting-001'], [], [], []));
      break;

    case KgMockScenario.INGEST_PARTIAL_FAILURE:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_WITH_ACTIVE));
      registry.register('api_ingest_meeting_to_knowledge_graph', ingestHandler([], ['meeting-001'], [], []));
      break;

    case KgMockScenario.INGEST_FULL_FAILURE:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_ingest_meeting_to_knowledge_graph', ingestHandler([], [], ['meeting-003'], []));
      break;

    case KgMockScenario.INGEST_ALL_ALREADY:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_ingest_meeting_to_knowledge_graph', ingestHandler([], [], [], ['meeting-001']));
      break;

    case KgMockScenario.QUERY_SUCCESS:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_query_knowledge_graph', queryHandler(['action items', 'decisions'], [], []));
      break;

    case KgMockScenario.QUERY_EMPTY:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_query_knowledge_graph', queryHandler([], ['nonexistent topic'], []));
      break;

    case KgMockScenario.QUERY_ERROR:
      registry.register('api_get_knowledge_graph_settings', constHandler(FIXTURE_SETTINGS_DEFAULT));
      registry.register('api_query_knowledge_graph', queryHandler([], [], ['error trigger']));
      break;

    default: {
      // Exhaustive check — if a new scenario is added, this will error at compile time.
      const _exhaustive: never = scenario;
      throw new Error(`Unknown scenario: ${_exhaustive}`);
    }
  }

  return registry;
}

// ── Mock Invoke Wrapper ──────────────────────────────────────────────

/**
 * A mock invoke function that delegates to a `KgInvokeRegistry`.
 *
 * Drop-in replacement for `@tauri-apps/api/core`'s `invoke`.
 */
export function createMockInvoke(registry: KgInvokeRegistry) {
  return async function invoke<T = unknown>(
    command: string,
    args?: Record<string, unknown>
  ): Promise<T> {
    const result = registry.resolve(command, args ?? {});
    return result as Promise<T>;
  };
}

// ── Installable Mock (for module patching) ───────────────────────────

/**
 * Creates a mock that can be installed/uninstalled.
 *
 * Install patches the `@tauri-apps/api/core` module's `invoke`
 * export so that all Tauri command calls in the test environment
 * hit the mock registry.
 *
 * In Bun tests, use `mock.module()` to patch the module before
 * importing the service.  In Playwright, inject via `page.addInitScript`.
 */
export interface InstallableKgMock {
  registry: KgInvokeRegistry;
  /** Get the mock invoke function. */
  getMockInvoke: () => ReturnType<typeof createMockInvoke>;
  /** Install the mock (patches the module). */
  install: () => void;
  /** Uninstall the mock (restores original). */
  uninstall: () => void;
  /** Reset call log. */
  reset: () => void;
  /** Get all recorded calls. */
  getCalls: () => { command: string; args: Record<string, unknown> }[];
}

export function createKgMock(
  scenario: KgMockScenario | KgMockScenario[]
): InstallableKgMock {
  const scenarios = Array.isArray(scenario) ? scenario : [scenario];
  const registry = new KgInvokeRegistry();

  for (const s of scenarios) {
    const subRegistry = buildScenarioRegistry(s);
    // Merge handlers — later scenarios override earlier ones
    for (const [cmd, handler] of subRegistry['handlers'] as Map<string, KgInvokeHandler>) {
      registry.register(cmd, handler);
    }
  }

  const mockInvoke = createMockInvoke(registry);
  let installed = false;
  let originalInvoke: typeof import('@tauri-apps/api/core').invoke | null = null;

  return {
    registry,
    getMockInvoke: () => mockInvoke,

    install: () => {
      if (installed) return;
      // In a real browser/test environment, this would patch the module.
      // For Bun tests, use `mock.module('@tauri-apps/api/core', ...)`.
      // For Playwright, use `page.addInitScript(...)`.
      // Here we just set a flag — the test harness is responsible for
      // actually wiring the mock into the module system.
      installed = true;
    },

    uninstall: () => {
      installed = false;
      originalInvoke = null;
      registry.resetCallLog();
    },

    reset: () => {
      registry.resetCallLog();
    },

    getCalls: () => {
      return registry.getCalls();
    },
  };
}

// ── Fixture Triggers ─────────────────────────────────────────────────

/**
 * Fixture trigger identifiers for QA flows.
 * These map to UI actions that should produce specific mock responses.
 */
export const KG_FIXTURE_TRIGGERS = {
  // Recording triggers
  MANUAL_RECORD_START: 'manual-record-start',
  SIDEBAR_AUTO_START_RECORDING: 'sidebar-auto-start-recording',
  SIDEBAR_START_RECORDING_FROM_SIDEBAR: 'sidebar-start-recording-from-sidebar',

  // Settings triggers
  SETTINGS_HEALTH: 'settings-health',
  SETTINGS_SAVE: 'settings-save',

  // Index/status triggers
  INDEX_SUCCESS: 'index-success',
  INDEX_RETRY: 'index-retry',
  INDEX_FAILURE: 'index-failure',

  // Query triggers
  QUERY_SUCCESS: 'query-success',
  QUERY_EMPTY: 'query-empty',
  QUERY_ERROR: 'query-error',

  // Meeting selection triggers
  MEETING_SELECT: 'meeting-select',
  MEETING_DESELECT: 'meeting-deselect',
} as const;

export type KgFixtureTrigger = (typeof KG_FIXTURE_TRIGGERS)[keyof typeof KG_FIXTURE_TRIGGERS];

/**
 * Map a fixture trigger to the scenario(s) it should activate.
 * Used by the QA harness to set up the correct mock state before
 * performing a UI action.
 */
export const TRIGGER_TO_SCENARIO: Record<KgFixtureTrigger, KgMockScenario[]> = {
  [KG_FIXTURE_TRIGGERS.MANUAL_RECORD_START]: [KgMockScenario.HEALTHY],
  [KG_FIXTURE_TRIGGERS.SIDEBAR_AUTO_START_RECORDING]: [KgMockScenario.HEALTHY],
  [KG_FIXTURE_TRIGGERS.SIDEBAR_START_RECORDING_FROM_SIDEBAR]: [KgMockScenario.HEALTHY],
  [KG_FIXTURE_TRIGGERS.SETTINGS_HEALTH]: [KgMockScenario.HEALTHY],
  [KG_FIXTURE_TRIGGERS.SETTINGS_SAVE]: [KgMockScenario.HEALTHY],
  [KG_FIXTURE_TRIGGERS.INDEX_SUCCESS]: [KgMockScenario.INGEST_SUCCESS],
  [KG_FIXTURE_TRIGGERS.INDEX_RETRY]: [KgMockScenario.INGEST_ALL_ALREADY],
  [KG_FIXTURE_TRIGGERS.INDEX_FAILURE]: [KgMockScenario.INGEST_FULL_FAILURE],
  [KG_FIXTURE_TRIGGERS.QUERY_SUCCESS]: [KgMockScenario.QUERY_SUCCESS],
  [KG_FIXTURE_TRIGGERS.QUERY_EMPTY]: [KgMockScenario.QUERY_EMPTY],
  [KG_FIXTURE_TRIGGERS.QUERY_ERROR]: [KgMockScenario.QUERY_ERROR],
  [KG_FIXTURE_TRIGGERS.MEETING_SELECT]: [KgMockScenario.SELECTED_PROFILE],
  [KG_FIXTURE_TRIGGERS.MEETING_DESELECT]: [KgMockScenario.NO_PROFILE],
};

// ── Recording Selector Fixture Data ──────────────────────────────────

/**
 * Recording selector fixture data for testing the recording flow.
 * These are the argument shapes passed to `start_recording`.
 */
export const RECORDING_SELECTOR_FIXTURES = {
  full: FIXTURE_RECORDING_SELECTOR_FULL,
  micOnly: FIXTURE_RECORDING_SELECTOR_MIC_ONLY,
  systemOnly: FIXTURE_RECORDING_SELECTOR_SYSTEM_ONLY,
  empty: FIXTURE_RECORDING_SELECTOR_EMPTY,
};

// ── Meeting Fixture Data ─────────────────────────────────────────────

export const MEETING_FIXTURES = {
  saved: FIXTURE_MEETING_SAVED,
  selectedProfile: FIXTURE_MEETING_SELECTED_PROFILE,
  noProfile: FIXTURE_MEETING_NO_PROFILE,
};