import { describe, expect, test, afterEach } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { TranscriptView } from '../../src/components/TranscriptView';
import { TooltipProvider } from '../../src/components/ui/tooltip';
import type { Transcript } from '../../src/types';

function makeTranscript(overrides: Partial<Transcript> = {}): Transcript {
  return {
    id: 't-001',
    text: 'Hello world',
    timestamp: '00:01',
    duration: 1.5,
    confidence: 0.9,
    ...overrides,
  };
}

interface MockStorage {
  data: Record<string, string>;
}

function createMockStorage(storage: MockStorage): Storage {
  return {
    get length() {
      return Object.keys(storage.data).length;
    },
    clear: () => { storage.data = {}; },
    getItem: (key: string) => storage.data[key] ?? null,
    key: (index: number) => Object.keys(storage.data)[index] ?? null,
    removeItem: (key: string) => { delete storage.data[key]; },
    setItem: (key: string, value: string) => { storage.data[key] = value; },
  };
}

function restoreGlobalProperty(name: 'window' | 'localStorage', descriptor: PropertyDescriptor | undefined): void {
  if (descriptor === undefined) {
    Reflect.deleteProperty(globalThis, name);
    return;
  }

  Object.defineProperty(globalThis, name, descriptor);
}

function installClientGlobals(storage: MockStorage): () => void {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const previousLocalStorage = Object.getOwnPropertyDescriptor(globalThis, 'localStorage');

  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: globalThis,
  });
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: createMockStorage(storage),
  });

  return () => {
    restoreGlobalProperty('window', previousWindow);
    restoreGlobalProperty('localStorage', previousLocalStorage);
  };
}

function renderTranscriptView(transcripts: Transcript[]): string {
  return renderToStaticMarkup(
    <TooltipProvider>
      <TranscriptView transcripts={transcripts} />
    </TooltipProvider>,
  );
}

describe('TranscriptView SSR determinism', () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    if (cleanup) {
      cleanup();
      cleanup = undefined;
    }
  });

  test('SSR output does not depend on showConfidenceIndicator storage', () => {
    const transcripts = [makeTranscript()];

    const htmlNoStorage = renderTranscriptView(transcripts);

    cleanup = installClientGlobals({
      data: { showConfidenceIndicator: 'false' },
    });

    const htmlWithStorageFalse = renderTranscriptView(transcripts);

    cleanup();
    cleanup = undefined;

    cleanup = installClientGlobals({
      data: { showConfidenceIndicator: 'true' },
    });

    const htmlWithStorageTrue = renderTranscriptView(transcripts);

    expect(htmlWithStorageFalse).toBe(htmlNoStorage);
    expect(htmlWithStorageTrue).toBe(htmlNoStorage);
  });

  test('SSR output renders transcript text regardless of localStorage state', () => {
    const transcripts = [makeTranscript({ text: 'Deterministic SSR content' })];

    cleanup = installClientGlobals({
      data: { showConfidenceIndicator: 'false' },
    });

    const html = renderTranscriptView(transcripts);

    expect(html).toContain('Deterministic SSR content');
  });
});
