import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import type { Glossary, GlossaryEntry, GlossarySyncResult } from '../../src/types/glossary';

// ── Fixtures ─────────────────────────────────────────────────────────

const FIXTURE_EMPTY_GLOSSARY: Glossary = { version: 1, entries: [] };

const FIXTURE_ENTRY_1: GlossaryEntry = {
  term: 'Parakeet',
  kind: 'project',
  pronunciation: 'pair-uh-keet',
  aliases: ['PK'],
  definition: 'Real-time speech recognition model by NVIDIA',
  notes: 'Used for local transcription',
};

const FIXTURE_ENTRY_2: GlossaryEntry = {
  term: 'API',
  kind: 'acronym',
  aliases: ['Application Programming Interface'],
};

const FIXTURE_GLOSSARY_WITH_ENTRIES: Glossary = {
  version: 1,
  entries: [FIXTURE_ENTRY_1, FIXTURE_ENTRY_2],
};

const FIXTURE_SYNC_SUCCESS: GlossarySyncResult = {
  profile_id: 'local-1',
  synced: true,
  track_id: 'track-abc',
  document_id: 'doc-xyz',
  error: null,
  skipped_reason: null,
};

// ── Mock Invoke ──────────────────────────────────────────────────────

const calls: { command: string; args: Record<string, unknown> }[] = [];

function defaultMockInvoke(command: string, args?: Record<string, unknown>): Promise<unknown> {
  calls.push({ command, args: args ?? {} });

  if (command === 'api_get_glossary') {
    return Promise.resolve(FIXTURE_EMPTY_GLOSSARY);
  }
  if (command === 'api_save_glossary') {
    return Promise.resolve(args?.glossary as Glossary);
  }
  if (command === 'api_sync_glossary_to_knowledge_graph') {
    return Promise.resolve(FIXTURE_SYNC_SUCCESS);
  }
  if (command === 'api_delete_glossary_from_knowledge_graph') {
    return Promise.resolve(FIXTURE_SYNC_SUCCESS);
  }

  return Promise.reject(new Error(`No mock handler registered for command: ${command}`));
}

type MockedInvoke = typeof defaultMockInvoke & {
  mockClear(): void;
  mockImplementation(fn: typeof defaultMockInvoke): void;
};

const mockInvoke = mock(defaultMockInvoke) as MockedInvoke;

mock.module('@tauri-apps/api/core', () => ({
  invoke: (...args: Parameters<typeof mockInvoke>) => mockInvoke(...args),
}));

// Mock toast to capture calls
const toastCalls: { type: string; message: string; description?: string }[] = [];
mock.module('sonner', () => ({
  toast: {
    success: (message: string, opts?: { description?: string }) => {
      toastCalls.push({ type: 'success', message, description: opts?.description });
    },
    error: (message: string, opts?: { description?: string }) => {
      toastCalls.push({ type: 'error', message, description: opts?.description });
    },
    info: (message: string, opts?: { description?: string }) => {
      toastCalls.push({ type: 'info', message, description: opts?.description });
    },
  },
}));

// Mock Dialog to render children directly in static markup
mock.module('@/components/ui/dialog', () => {
  const React = require('react');
  return {
    Dialog: ({ children, open }: { children: React.ReactNode; open?: boolean }) =>
      open ? React.createElement('div', { 'data-testid': 'dialog', className: 'dialog-mock' }, children) : null,
    DialogContent: ({ children }: { children: React.ReactNode }) =>
      React.createElement('div', { className: 'dialog-content-mock' }, children),
    DialogHeader: ({ children }: { children: React.ReactNode }) =>
      React.createElement('div', { className: 'dialog-header-mock' }, children),
    DialogTitle: ({ children }: { children: React.ReactNode }) =>
      React.createElement('h2', { className: 'dialog-title-mock' }, children),
    DialogDescription: ({ children }: { children: React.ReactNode }) =>
      React.createElement('p', { className: 'dialog-description-mock' }, children),
    DialogFooter: ({ children }: { children: React.ReactNode }) =>
      React.createElement('div', { className: 'dialog-footer-mock' }, children),
  };
});

// ── Import components after mocks ────────────────────────────────────

const { GlossarySettings, persistGlossary, syncGlossary, deleteGlossaryFromKg } = await import('../../src/components/GlossarySettings');
const { GlossaryEditor, computeNextGlossary, computeGlossaryAfterDelete } = await import('../../src/components/GlossaryEditor');
const { GlossaryEntryDialog, emptyForm, entryToForm, formToEntry, validateForm } = await import('../../src/components/GlossaryEntryDialog');
const { GlossaryService } = await import('../../src/services/glossaryService');

// ── Tests ────────────────────────────────────────────────────────────

describe('GlossarySettings', () => {
  beforeEach(() => {
    calls.length = 0;
    toastCalls.length = 0;
    resetMock();
  });

  afterEach(() => {
    calls.length = 0;
    toastCalls.length = 0;
  });

  test('shows loading state on initial render without prop', () => {
    const html = renderToStaticMarkup(<GlossarySettings />);
    expect(html).toContain('Loading glossary');
    expect(html).toContain('animate-spin');
  });

  test('shows empty state when initialGlossary is empty', () => {
    const html = renderToStaticMarkup(<GlossarySettings initialGlossary={FIXTURE_EMPTY_GLOSSARY} />);
    expect(html).toContain('No Glossary Entries');
    expect(html).toContain('Add First Entry');
  });

  test('shows entries when initialGlossary has entries', () => {
    const html = renderToStaticMarkup(<GlossarySettings initialGlossary={FIXTURE_GLOSSARY_WITH_ENTRIES} />);
    expect(html).toContain('Parakeet');
    expect(html).toContain('project');
    expect(html).toContain('API');
    expect(html).toContain('acronym');
  });

  test('renders sync and delete-from-kg buttons', () => {
    const html = renderToStaticMarkup(<GlossarySettings initialGlossary={FIXTURE_EMPTY_GLOSSARY} />);
    expect(html).toContain('Sync to KG');
    expect(html).toContain('Delete from KG');
  });

  test('renders save button when entries exist', () => {
    const html = renderToStaticMarkup(<GlossarySettings initialGlossary={FIXTURE_GLOSSARY_WITH_ENTRIES} />);
    expect(html).toContain('Save Glossary');
  });

  test('does not render save button when glossary is empty', () => {
    const html = renderToStaticMarkup(<GlossarySettings initialGlossary={FIXTURE_EMPTY_GLOSSARY} />);
    expect(html).not.toContain('Save Glossary');
  });
});

describe('GlossaryEditor', () => {
  test('renders empty state with add button', () => {
    const html = renderToStaticMarkup(
      <GlossaryEditor
        glossary={FIXTURE_EMPTY_GLOSSARY}
        onChange={() => {}}
        onSave={() => {}}
        onSync={() => {}}
        onDeleteFromKg={() => {}}
        saving={false}
        syncing={false}
        deletingFromKg={false}
      />
    );
    expect(html).toContain('No Glossary Entries');
    expect(html).toContain('Add First Entry');
  });

  test('renders entries with edit and delete buttons', () => {
    const html = renderToStaticMarkup(
      <GlossaryEditor
        glossary={FIXTURE_GLOSSARY_WITH_ENTRIES}
        onChange={() => {}}
        onSave={() => {}}
        onSync={() => {}}
        onDeleteFromKg={() => {}}
        saving={false}
        syncing={false}
        deletingFromKg={false}
      />
    );
    expect(html).toContain('Parakeet');
    expect(html).toContain('Edit Parakeet');
    expect(html).toContain('Delete Parakeet');
    expect(html).toContain('API');
    expect(html).toContain('Edit API');
    expect(html).toContain('Delete API');
  });

  test('disables buttons when saving is true', () => {
    const html = renderToStaticMarkup(
      <GlossaryEditor
        glossary={FIXTURE_GLOSSARY_WITH_ENTRIES}
        onChange={() => {}}
        onSave={() => {}}
        onSync={() => {}}
        onDeleteFromKg={() => {}}
        saving={true}
        syncing={false}
        deletingFromKg={false}
      />
    );
    expect(html).toContain('disabled');
  });

  test('shows loading spinner on sync button when syncing', () => {
    const html = renderToStaticMarkup(
      <GlossaryEditor
        glossary={FIXTURE_GLOSSARY_WITH_ENTRIES}
        onChange={() => {}}
        onSave={() => {}}
        onSync={() => {}}
        onDeleteFromKg={() => {}}
        saving={false}
        syncing={true}
        deletingFromKg={false}
      />
    );
    expect(html).toContain('animate-spin');
  });
});

describe('GlossaryEntryDialog', () => {
  test('renders add dialog with all form fields', () => {
    const html = renderToStaticMarkup(
      <GlossaryEntryDialog
        open={true}
        onOpenChange={() => {}}
        editingIndex={null}
        form={emptyForm()}
        formError={null}
        onFormChange={() => {}}
        onSave={() => {}}
      />
    );
    expect(html).toContain('Add Glossary Entry');
    expect(html).toContain('g-term');
    expect(html).toContain('g-kind');
    expect(html).toContain('g-pronunciation');
    expect(html).toContain('g-aliases');
    expect(html).toContain('g-definition');
    expect(html).toContain('g-notes');
    expect(html).toContain('Add Entry');
  });

  test('renders edit dialog with update button', () => {
    const html = renderToStaticMarkup(
      <GlossaryEntryDialog
        open={true}
        onOpenChange={() => {}}
        editingIndex={0}
        form={entryToForm(FIXTURE_ENTRY_1)}
        formError={null}
        onFormChange={() => {}}
        onSave={() => {}}
      />
    );
    expect(html).toContain('Edit Entry');
    expect(html).toContain('Update Entry');
    expect(html).toContain('Parakeet');
  });

  test('shows validation error when formError is set', () => {
    const html = renderToStaticMarkup(
      <GlossaryEntryDialog
        open={true}
        onOpenChange={() => {}}
        editingIndex={null}
        form={emptyForm()}
        formError="Term is required"
        onFormChange={() => {}}
        onSave={() => {}}
      />
    );
    expect(html).toContain('Term is required');
  });

  test('renders kind select trigger in dialog', () => {
    const html = renderToStaticMarkup(
      <GlossaryEntryDialog
        open={true}
        onOpenChange={() => {}}
        editingIndex={null}
        form={emptyForm()}
        formError={null}
        onFormChange={() => {}}
        onSave={() => {}}
      />
    );
    expect(html).toContain('g-kind');
    expect(html).toContain('Kind');
    expect(html).toContain('role="combobox"');
  });
});

describe('Glossary form helpers', () => {
  test('emptyForm returns default form state', () => {
    const form = emptyForm();
    expect(form.term).toBe('');
    expect(form.kind).toBe('other');
    expect(form.pronunciation).toBe('');
    expect(form.aliases).toBe('');
    expect(form.definition).toBe('');
    expect(form.notes).toBe('');
  });

  test('entryToForm converts entry to form state', () => {
    const form = entryToForm(FIXTURE_ENTRY_1);
    expect(form.term).toBe('Parakeet');
    expect(form.kind).toBe('project');
    expect(form.pronunciation).toBe('pair-uh-keet');
    expect(form.aliases).toBe('PK');
    expect(form.definition).toBe('Real-time speech recognition model by NVIDIA');
    expect(form.notes).toBe('Used for local transcription');
  });

  test('entryToForm handles optional fields', () => {
    const form = entryToForm(FIXTURE_ENTRY_2);
    expect(form.term).toBe('API');
    expect(form.kind).toBe('acronym');
    expect(form.pronunciation).toBe('');
    expect(form.aliases).toBe('Application Programming Interface');
    expect(form.definition).toBe('');
    expect(form.notes).toBe('');
  });

  test('formToEntry converts form to entry with trimming', () => {
    const form: import('../../src/components/GlossaryEntryDialog').FormState = {
      term: '  Parakeet  ',
      kind: 'project',
      pronunciation: '  pair-uh-keet  ',
      aliases: 'PK,  Parakeet TDT ',
      definition: '  A model  ',
      notes: '  Some notes  ',
    };
    const entry = formToEntry(form);
    expect(entry.term).toBe('Parakeet');
    expect(entry.kind).toBe('project');
    expect(entry.pronunciation).toBe('pair-uh-keet');
    expect(entry.aliases).toEqual(['PK', 'Parakeet TDT']);
    expect(entry.definition).toBe('A model');
    expect(entry.notes).toBe('Some notes');
  });

  test('formToEntry omits optional fields when empty', () => {
    const form = emptyForm();
    form.term = 'API';
    form.kind = 'acronym';
    const entry = formToEntry(form);
    expect(entry.term).toBe('API');
    expect(entry.pronunciation).toBeUndefined();
    expect(entry.definition).toBeUndefined();
    expect(entry.notes).toBeUndefined();
    expect(entry.aliases).toEqual([]);
  });
});

describe('validateForm', () => {
  test('returns valid for non-empty term', () => {
    const result = validateForm({ term: 'Parakeet', kind: 'project', pronunciation: '', aliases: '', definition: '', notes: '' });
    expect(result.valid).toBe(true);
    expect(result.error).toBeNull();
  });

  test('returns invalid for empty term', () => {
    const result = validateForm({ term: '', kind: 'other', pronunciation: '', aliases: '', definition: '', notes: '' });
    expect(result.valid).toBe(false);
    expect(result.error).toBe('Term is required');
  });

  test('returns invalid for whitespace-only term', () => {
    const result = validateForm({ term: '   ', kind: 'other', pronunciation: '', aliases: '', definition: '', notes: '' });
    expect(result.valid).toBe(false);
    expect(result.error).toBe('Term is required');
  });
});

describe('computeNextGlossary', () => {
  test('adds new entry when editingIndex is null', () => {
    const form = emptyForm();
    form.term = 'New Term';
    form.kind = 'person';
    const next = computeNextGlossary(FIXTURE_EMPTY_GLOSSARY, form, null);
    expect(next.entries.length).toBe(1);
    expect(next.entries[0].term).toBe('New Term');
    expect(next.entries[0].kind).toBe('person');
  });

  test('edits existing entry when editingIndex is provided', () => {
    const form = emptyForm();
    form.term = 'Updated Parakeet';
    form.kind = 'component';
    const next = computeNextGlossary(FIXTURE_GLOSSARY_WITH_ENTRIES, form, 0);
    expect(next.entries.length).toBe(2);
    expect(next.entries[0].term).toBe('Updated Parakeet');
    expect(next.entries[0].kind).toBe('component');
    expect(next.entries[1].term).toBe('API');
  });

  test('preserves version when adding entry', () => {
    const form = emptyForm();
    form.term = 'New';
    const next = computeNextGlossary(FIXTURE_EMPTY_GLOSSARY, form, null);
    expect(next.version).toBe(1);
  });
});

describe('computeGlossaryAfterDelete', () => {
  test('removes entry at specified index', () => {
    const next = computeGlossaryAfterDelete(FIXTURE_GLOSSARY_WITH_ENTRIES, 0);
    expect(next.entries.length).toBe(1);
    expect(next.entries[0].term).toBe('API');
  });

  test('preserves version after delete', () => {
    const next = computeGlossaryAfterDelete(FIXTURE_GLOSSARY_WITH_ENTRIES, 0);
    expect(next.version).toBe(1);
  });

  test('returns empty glossary when deleting last entry', () => {
    const glossary: Glossary = { version: 1, entries: [FIXTURE_ENTRY_1] };
    const next = computeGlossaryAfterDelete(glossary, 0);
    expect(next.entries.length).toBe(0);
  });
});

describe('persistGlossary', () => {
  beforeEach(() => {
    calls.length = 0;
    toastCalls.length = 0;
    resetMock();
  });

  afterEach(() => {
    calls.length = 0;
    toastCalls.length = 0;
  });

  test('calls service.saveGlossary and returns success', async () => {
    const service = new GlossaryService();
    const result = await persistGlossary(FIXTURE_GLOSSARY_WITH_ENTRIES, service);
    expect(result.success).toBe(true);
    expect(result.glossary).toEqual(FIXTURE_GLOSSARY_WITH_ENTRIES);
    const saveCalls = calls.filter((c) => c.command === 'api_save_glossary');
    expect(saveCalls.length).toBe(1);
    expect(saveCalls[0].args).toEqual({ glossary: FIXTURE_GLOSSARY_WITH_ENTRIES });
  });

  test('returns error when glossary has blank term without calling service', async () => {
    const service = new GlossaryService();
    const badGlossary: Glossary = {
      version: 1,
      entries: [{ term: '', kind: 'other', aliases: [] }],
    };
    const result = await persistGlossary(badGlossary, service);
    expect(result.success).toBe(false);
    expect(result.error).toBe('Cannot save: one or more entries have a blank term');
    const saveCalls = calls.filter((c) => c.command === 'api_save_glossary');
    expect(saveCalls.length).toBe(0);
  });

  test('returns error when service throws', async () => {
    const service = new GlossaryService();
    mockInvoke.mockImplementation((command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args: args ?? {} });
      if (command === 'api_save_glossary') {
        return Promise.reject(new Error('validation failed: term must be non-empty'));
      }
      return defaultMockInvoke(command, args);
    });
    const result = await persistGlossary(FIXTURE_GLOSSARY_WITH_ENTRIES, service);
    expect(result.success).toBe(false);
    expect(result.error).toBe('validation failed: term must be non-empty');
  });
});

function resetMock() {
  mockInvoke.mockClear();
  mockInvoke.mockImplementation(defaultMockInvoke);
}

describe('syncGlossary', () => {
  beforeEach(() => {
    calls.length = 0;
    resetMock();
  });

  afterEach(() => {
    calls.length = 0;
  });

  test('calls service.syncGlossaryToKnowledgeGraph', async () => {
    const service = new GlossaryService();
    const result = await syncGlossary(service);
    expect(result.synced).toBe(true);
    const syncCalls = calls.filter((c) => c.command === 'api_sync_glossary_to_knowledge_graph');
    expect(syncCalls.length).toBe(1);
    expect(syncCalls[0].args).toEqual({});
  });
});

describe('deleteGlossaryFromKg', () => {
  beforeEach(() => {
    calls.length = 0;
    resetMock();
  });

  afterEach(() => {
    calls.length = 0;
  });

  test('calls service.deleteGlossaryFromKnowledgeGraph', async () => {
    const service = new GlossaryService();
    const result = await deleteGlossaryFromKg(service);
    expect(result.synced).toBe(true);
    const deleteCalls = calls.filter((c) => c.command === 'api_delete_glossary_from_knowledge_graph');
    expect(deleteCalls.length).toBe(1);
    expect(deleteCalls[0].args).toEqual({});
  });
});

describe('GlossarySettings service integration', () => {
  beforeEach(() => {
    calls.length = 0;
    resetMock();
  });

  afterEach(() => {
    calls.length = 0;
  });

  test('initialGlossary prop skips service call on mount', () => {
    renderToStaticMarkup(<GlossarySettings initialGlossary={FIXTURE_GLOSSARY_WITH_ENTRIES} />);
    expect(calls.filter((c) => c.command === 'api_get_glossary').length).toBe(0);
  });
});
