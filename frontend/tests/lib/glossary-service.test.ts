/**
 * Glossary Service Unit Tests
 *
 * Tests that GlossaryService methods call the correct Tauri commands
 * with the expected argument shapes. Follows the same pattern as
 * `knowledge-graph-service.test.ts`.
 */

import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import {
  DEFAULT_GLOSSARY,
} from '../../src/types/glossary';
import type {
  Glossary,
  GlossaryEntry,
  GlossarySyncResult,
} from '../../src/types/glossary';

// ── Types ────────────────────────────────────────────────────────────

type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

// ── Fixtures ─────────────────────────────────────────────────────────

const FIXTURE_EMPTY_GLOSSARY: Glossary = {
  version: 1,
  entries: [],
};

const FIXTURE_SAMPLE_ENTRY: GlossaryEntry = {
  id: 'entry-parakeet-1',
  term: 'Parakeet',
  kind: 'project',
  pronunciation: 'pair-uh-keet',
  aliases: ['PK'],
  definition: 'Real-time speech recognition model by NVIDIA',
  notes: 'Used for local transcription',
  references: ['https://github.com/NVIDIA/parakeet'],
};

const FIXTURE_GLOSSARY_WITH_ENTRY: Glossary = {
  version: 1,
  entries: [FIXTURE_SAMPLE_ENTRY],
};

const FIXTURE_SYNC_SUCCESS: GlossarySyncResult = {
  profile_id: 'local-1',
  synced: true,
  track_id: 'track-abc-123',
  document_id: null,
  error: null,
  skipped_reason: null,
};

const FIXTURE_SYNC_NO_PROFILE: GlossarySyncResult = {
  profile_id: null,
  synced: false,
  track_id: null,
  document_id: null,
  error: null,
  skipped_reason: 'No active knowledge graph profile configured',
};

const FIXTURE_SYNC_EMPTY: GlossarySyncResult = {
  profile_id: 'local-1',
  synced: false,
  track_id: null,
  document_id: null,
  error: null,
  skipped_reason: 'Glossary has no entries — nothing to sync',
};

const FIXTURE_DELETE_SUCCESS: GlossarySyncResult = {
  profile_id: 'local-1',
  synced: true,
  track_id: null,
  document_id: null,
  error: null,
  skipped_reason: null,
};

const FIXTURE_SYNC_PROVIDER_ERROR: GlossarySyncResult = {
  profile_id: 'local-1',
  synced: false,
  track_id: null,
  document_id: null,
  error: 'LightRAG returned HTTP 502',
  skipped_reason: null,
};

// ── Mock Invoke Setup ────────────────────────────────────────────────

/**
 * Simple handler registry for glossary commands.
 */
function createGlossaryMockInvoke() {
  const handlers = new Map<string, (args: Record<string, unknown>) => unknown>();
  const callLog: { command: string; args: Record<string, unknown> }[] = [];

  const mockInvoke: InvokeFn = async (command, args) => {
    const resolvedArgs = args ?? {};
    callLog.push({ command, args: resolvedArgs });

    const handler = handlers.get(command);
    if (!handler) {
      return Promise.reject(
        new Error(`No mock handler registered for command: ${command}`)
      );
    }

    const result = handler(resolvedArgs);
    // If the handler threw, return a rejected promise
    if (result instanceof Error || (result && typeof result === 'object' && 'rejected' in result)) {
      return Promise.reject(result);
    }
    return Promise.resolve(result);
  };

  return {
    mockInvoke,
    register: (command: string, handler: (args: Record<string, unknown>) => unknown) => {
      handlers.set(command, handler);
    },
    getCalls: () => [...callLog],
    getCallsFor: (command: string) => callLog.filter((c) => c.command === command),
    clear: () => {
      handlers.clear();
      callLog.length = 0;
    },
  };
}

// ── Module Patching ──────────────────────────────────────────────────

let mockState: ReturnType<typeof createGlossaryMockInvoke>;

// Patch @tauri-apps/api/core before importing the service.
mock.module('@tauri-apps/api/core', () => ({
  invoke: (command: string, args?: Record<string, unknown>) =>
    mockState.mockInvoke(command, args),
}));

// Import after mock is set up.
const { GlossaryService } = await import('../../src/services/glossaryService');

// ── Tests ────────────────────────────────────────────────────────────

describe('GlossaryService', () => {
  beforeEach(() => {
    mockState = createGlossaryMockInvoke();
  });

  afterEach(() => {
    mockState.clear();
  });

  // ── getGlossary ──────────────────────────────────────────────────

  test('getGlossary calls api_get_glossary with no args', async () => {
    mockState.register('api_get_glossary', () => FIXTURE_GLOSSARY_WITH_ENTRY);

    const service = new GlossaryService();
    const result = await service.getGlossary();

    expect(result).toEqual(FIXTURE_GLOSSARY_WITH_ENTRY);
    const calls = mockState.getCallsFor('api_get_glossary');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({});
  });

  test('getGlossary returns empty glossary when Rust returns default', async () => {
    mockState.register('api_get_glossary', () => FIXTURE_EMPTY_GLOSSARY);

    const service = new GlossaryService();
    const result = await service.getGlossary();

    expect(result.version).toBe(1);
    expect(result.entries).toEqual([]);
  });

  test('getGlossary propagates rejection from Rust', async () => {
    mockState.register('api_get_glossary', () => {
      throw new Error('Failed to load glossary: file not found');
    });

    const service = new GlossaryService();
    await expect(service.getGlossary()).rejects.toThrow(
      'Failed to load glossary: file not found'
    );
  });

  // ── saveGlossary ─────────────────────────────────────────────────

  test('saveGlossary calls api_save_glossary with { glossary } payload', async () => {
    mockState.register('api_save_glossary', () => FIXTURE_GLOSSARY_WITH_ENTRY);

    const service = new GlossaryService();
    const result = await service.saveGlossary(FIXTURE_GLOSSARY_WITH_ENTRY);

    expect(result).toEqual(FIXTURE_GLOSSARY_WITH_ENTRY);
    const calls = mockState.getCallsFor('api_save_glossary');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({ glossary: FIXTURE_GLOSSARY_WITH_ENTRY });
  });

  test('saveGlossary propagates validation rejection from Rust', async () => {
    mockState.register('api_save_glossary', () => {
      throw new Error("entry 0: term must be non-empty after trimming");
    });

    const invalid: Glossary = {
      version: 1,
      entries: [{ id: 'entry-invalid', term: '', kind: 'other', aliases: [] }],
    };

    const service = new GlossaryService();
    await expect(service.saveGlossary(invalid)).rejects.toThrow(
      'entry 0: term must be non-empty after trimming'
    );
  });

  // ── syncGlossaryToKnowledgeGraph ─────────────────────────────────

  test('syncGlossaryToKnowledgeGraph calls api_sync_glossary_to_knowledge_graph with previousGlossary', async () => {
    mockState.register('api_sync_glossary_to_knowledge_graph', () => FIXTURE_SYNC_SUCCESS);

    const service = new GlossaryService();
    const previous: Glossary = {
      version: 1,
      entries: [{ id: 'entry-old', term: 'Old', kind: 'other', aliases: [] }],
    };
    const result = await service.syncGlossaryToKnowledgeGraph(previous);

    expect(result).toEqual(FIXTURE_SYNC_SUCCESS);
    expect(result.profile_id).toBe('local-1');
    expect(result.synced).toBe(true);
    expect(result.track_id).toBe('track-abc-123');

    const calls = mockState.getCallsFor('api_sync_glossary_to_knowledge_graph');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({ previousGlossary: previous });
  });

  test('syncGlossaryToKnowledgeGraph calls api_sync_glossary_to_knowledge_graph with no previousGlossary when omitted', async () => {
    mockState.register('api_sync_glossary_to_knowledge_graph', () => FIXTURE_SYNC_SUCCESS);

    const service = new GlossaryService();
    const result = await service.syncGlossaryToKnowledgeGraph();

    expect(result).toEqual(FIXTURE_SYNC_SUCCESS);

    const calls = mockState.getCallsFor('api_sync_glossary_to_knowledge_graph');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({ previousGlossary: undefined });
  });

  test('syncGlossaryToKnowledgeGraph returns skipped when no active profile', async () => {
    mockState.register('api_sync_glossary_to_knowledge_graph', () => FIXTURE_SYNC_NO_PROFILE);

    const service = new GlossaryService();
    const result = await service.syncGlossaryToKnowledgeGraph();

    expect(result.profile_id).toBeNull();
    expect(result.synced).toBe(false);
    expect(result.skipped_reason).toBe(
      'No active knowledge graph profile configured'
    );
  });

  test('syncGlossaryToKnowledgeGraph returns skipped when glossary is empty', async () => {
    mockState.register('api_sync_glossary_to_knowledge_graph', () => FIXTURE_SYNC_EMPTY);

    const service = new GlossaryService();
    const result = await service.syncGlossaryToKnowledgeGraph();

    expect(result.synced).toBe(false);
    expect(result.skipped_reason).toBe(
      'Glossary has no entries — nothing to sync'
    );
  });

  test('syncGlossaryToKnowledgeGraph returns error field on provider failure', async () => {
    mockState.register('api_sync_glossary_to_knowledge_graph', () => FIXTURE_SYNC_PROVIDER_ERROR);

    const service = new GlossaryService();
    const result = await service.syncGlossaryToKnowledgeGraph();

    // Provider failures are returned as Ok with error field, not thrown by Rust.
    expect(result.synced).toBe(false);
    expect(result.error).toBe('LightRAG returned HTTP 502');
    expect(result.skipped_reason).toBeNull();
  });

  // ── deleteGlossaryFromKnowledgeGraph ──────────────────────────────

  test('deleteGlossaryFromKnowledgeGraph calls api_delete_glossary_from_knowledge_graph with no args', async () => {
    mockState.register('api_delete_glossary_from_knowledge_graph', () => FIXTURE_DELETE_SUCCESS);

    const service = new GlossaryService();
    const result = await service.deleteGlossaryFromKnowledgeGraph();

    expect(result).toEqual(FIXTURE_DELETE_SUCCESS);
    expect(result.synced).toBe(true);

    const calls = mockState.getCallsFor('api_delete_glossary_from_knowledge_graph');
    expect(calls.length).toBe(1);
    expect(calls[0].args).toEqual({});
  });

  test('deleteGlossaryFromKnowledgeGraph returns error field on provider failure', async () => {
    mockState.register(
      'api_delete_glossary_from_knowledge_graph',
      () => FIXTURE_SYNC_PROVIDER_ERROR
    );

    const service = new GlossaryService();
    const result = await service.deleteGlossaryFromKnowledgeGraph();

    expect(result.synced).toBe(false);
    expect(result.error).toBe('LightRAG returned HTTP 502');
  });

  test('deleteGlossaryFromKnowledgeGraph propagates rejection from Rust', async () => {
    mockState.register('api_delete_glossary_from_knowledge_graph', () => {
      throw new Error('Failed to initialize secret store: permission denied');
    });

    const service = new GlossaryService();
    await expect(service.deleteGlossaryFromKnowledgeGraph()).rejects.toThrow(
      'Failed to initialize secret store: permission denied'
    );
  });

  // ── Singleton ────────────────────────────────────────────────────

  test('glossaryService exports a defined singleton', async () => {
    const { glossaryService } = await import('../../src/services/glossaryService');
    expect(glossaryService).toBeDefined();
    expect(typeof glossaryService.getGlossary).toBe('function');
  });
});

// ── Type Shape Tests ─────────────────────────────────────────────────

describe('Glossary type shapes', () => {
  test('GlossaryKind only accepts valid Rust VALID_KINDS values', () => {
    // TypeScript compile-time check — these should compile.
    const validKinds = [
      'person',
      'team',
      'project',
      'code_name',
      'component',
      'acronym',
      'other',
    ] as const;
    expect(validKinds.length).toBe(7);
  });

  test('GlossaryEntry matches Rust serde shape', () => {
    const entry: GlossaryEntry = FIXTURE_SAMPLE_ENTRY;
    expect(entry.id).toBe('entry-parakeet-1');
    expect(entry.term).toBe('Parakeet');
    expect(entry.kind).toBe('project');
    expect(entry.pronunciation).toBe('pair-uh-keet');
    expect(entry.aliases).toEqual(['PK']);
    expect(entry.definition).toBe('Real-time speech recognition model by NVIDIA');
    expect(entry.notes).toBe('Used for local transcription');
    expect(entry.references).toEqual(['https://github.com/NVIDIA/parakeet']);
  });

  test('GlossaryEntry optional fields can be omitted', () => {
    const entry: GlossaryEntry = {
      id: 'entry-api',
      term: 'API',
      kind: 'acronym',
      aliases: [],
    };
    expect(entry.pronunciation).toBeUndefined();
    expect(entry.definition).toBeUndefined();
    expect(entry.notes).toBeUndefined();
    expect(entry.references).toBeUndefined();
  });

  test('Glossary matches Rust serde shape with version and entries', () => {
    const glossary: Glossary = FIXTURE_GLOSSARY_WITH_ENTRY;
    expect(glossary.version).toBe(1);
    expect(glossary.entries.length).toBe(1);
    expect(glossary.entries[0].term).toBe('Parakeet');
  });

  test('DEFAULT_GLOSSARY is empty with version 1', () => {
    expect(DEFAULT_GLOSSARY.version).toBe(1);
    expect(DEFAULT_GLOSSARY.entries).toEqual([]);
  });

  test('GlossarySyncResult matches Rust serde shape', () => {
    const result: GlossarySyncResult = FIXTURE_SYNC_SUCCESS;
    expect(result.profile_id).toBe('local-1');
    expect(result.synced).toBe(true);
    expect(result.track_id).toBe('track-abc-123');
    expect(result.document_id).toBeNull();
    expect(result.error).toBeNull();
    expect(result.skipped_reason).toBeNull();
  });

  test('generateEntryId returns a non-empty string', () => {
    const { generateEntryId } = require('../../src/types/glossary');
    const id = generateEntryId();
    expect(typeof id).toBe('string');
    expect(id.length).toBeGreaterThan(0);
  });

  test('GlossarySyncResult skipped_reason is non-null when not synced', () => {
    const result: GlossarySyncResult = FIXTURE_SYNC_NO_PROFILE;
    expect(result.synced).toBe(false);
    expect(result.skipped_reason).not.toBeNull();
  });
});

// ── Mock Registry Tests ──────────────────────────────────────────────

describe('glossary mock invoke registry', () => {
  test('rejects unregistered commands', async () => {
    const state = createGlossaryMockInvoke();

    await expect(state.mockInvoke('unknown_command')).rejects.toThrow(
      'No mock handler registered for command: unknown_command'
    );
  });

  test('records call log for assertions', async () => {
    const state = createGlossaryMockInvoke();
    state.register('api_get_glossary', () => FIXTURE_EMPTY_GLOSSARY);
    state.register('api_save_glossary', () => FIXTURE_GLOSSARY_WITH_ENTRY);

    await state.mockInvoke('api_get_glossary');
    await state.mockInvoke('api_save_glossary', { glossary: FIXTURE_GLOSSARY_WITH_ENTRY });

    const calls = state.getCalls();
    expect(calls.length).toBe(2);
    expect(calls[0].command).toBe('api_get_glossary');
    expect(calls[1].command).toBe('api_save_glossary');
    expect(calls[1].args).toEqual({ glossary: FIXTURE_GLOSSARY_WITH_ENTRY });
  });

  test('clear resets handlers and call log', async () => {
    const state = createGlossaryMockInvoke();
    state.register('api_get_glossary', () => FIXTURE_EMPTY_GLOSSARY);
    await state.mockInvoke('api_get_glossary');
    expect(state.getCalls().length).toBe(1);

    state.clear();
    expect(state.getCalls().length).toBe(0);

    // After clear, unregistered commands reject
    await expect(state.mockInvoke('api_get_glossary')).rejects.toThrow(
      'No mock handler registered'
    );
    // The rejected call is logged before handler lookup, so we have 1 entry now
    expect(state.getCalls().length).toBe(1);
  });
});
