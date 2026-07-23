import { describe, expect, test, afterEach } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import {
  ConfigProvider,
  reconcileCalendarDerivedItems,
  reconcileCalendarPermissionStatus,
  useConfig,
} from '../../src/contexts/ConfigContext';
import { DEFAULT_BETA_FEATURES } from '../../src/types/betaFeatures';

function ConfigProbe(): string {
  const config = useConfig();
  return [
    config.selectedLanguage,
    String(config.showConfidenceIndicator),
    String(config.isAutoSummary),
    String(config.betaFeatures.importAndRetranscribe),
  ].join('|');
}

const DEFAULT_PROBE_OUTPUT = [
  'auto',
  'true',
  'false',
  String(DEFAULT_BETA_FEATURES.importAndRetranscribe),
].join('|');

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

describe('ConfigProvider SSR determinism', () => {
  let cleanup: (() => void) | undefined;

  afterEach(() => {
    if (cleanup) {
      cleanup();
      cleanup = undefined;
    }
  });

  test('SSR output uses defaults when localStorage is empty', () => {
    cleanup = installClientGlobals({ data: {} });
    const html = renderToStaticMarkup(
      <ConfigProvider>
        <ConfigProbe />
      </ConfigProvider>,
    );
    expect(html).toBe(DEFAULT_PROBE_OUTPUT);
  });

  test('SSR output does not depend on client localStorage values', () => {
    cleanup = installClientGlobals({
      data: {
        primaryLanguage: 'es',
        showConfidenceIndicator: 'false',
        isAutoSummary: 'true',
        betaFeatures: JSON.stringify({ importAndRetranscribe: false }),
      },
    });

    const html = renderToStaticMarkup(
      <ConfigProvider>
        <ConfigProbe />
      </ConfigProvider>,
    );

    expect(html).toBe(DEFAULT_PROBE_OUTPUT);
  });

  test('SSR output is identical regardless of localStorage state', () => {
    const htmlNoStorage = renderToStaticMarkup(
      <ConfigProvider>
        <ConfigProbe />
      </ConfigProvider>,
    );

    cleanup = installClientGlobals({
      data: {
        primaryLanguage: 'fr',
        showConfidenceIndicator: 'false',
        isAutoSummary: 'true',
        betaFeatures: JSON.stringify({ importAndRetranscribe: false }),
      },
    });

    const htmlWithStorage = renderToStaticMarkup(
      <ConfigProvider>
        <ConfigProbe />
      </ConfigProvider>,
    );

    expect(htmlWithStorage).toBe(htmlNoStorage);
  });
});

describe('calendar permission reconciliation', () => {
  test('keeps granted permission when refresh returns transient unknown status', () => {
    expect(reconcileCalendarPermissionStatus('unknown', null, 'authorized')).toBe('authorized');
  });

  test('keeps granted permission when refresh returns transient not determined status', () => {
    expect(reconcileCalendarPermissionStatus('not_determined', null, 'full_access')).toBe('full_access');
  });

  test('does not keep granted permission when backend reports explicit denial', () => {
    expect(reconcileCalendarPermissionStatus('denied', null, 'authorized')).toBe('denied');
  });
});

describe('calendar derived item reconciliation', () => {
  const previous = [{ id: 'cal-work', title: 'Work' }];
  const next = [{ id: 'cal-personal', title: 'Personal' }];

  test('keeps previous non-empty items when readable refresh returns transient empty list', () => {
    expect(reconcileCalendarDerivedItems(previous, [], 'authorized')).toBe(previous);
  });

  test('allows first readable empty result to stay empty', () => {
    expect(reconcileCalendarDerivedItems([], [], 'authorized')).toEqual([]);
  });

  test('replaces previous items when readable refresh returns non-empty list', () => {
    expect(reconcileCalendarDerivedItems(previous, next, 'full_access')).toBe(next);
  });

  test('clears previous items when permission is no longer readable', () => {
    expect(reconcileCalendarDerivedItems(previous, [], 'denied')).toEqual([]);
  });

  test('clears fetched items when permission is no longer readable', () => {
    expect(reconcileCalendarDerivedItems(previous, next, 'restricted')).toEqual([]);
  });
});
