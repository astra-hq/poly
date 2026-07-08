/**
 * Calendar Mock Invoke Harness
 *
 * Provides a deterministic mock of Tauri's `invoke` function for
 * browser QA (Playwright) and unit tests. Instead of calling the
 * Rust backend, the mock looks up the command name in a handler map
 * and returns the registered fixture data.
 *
 * Mirrors the pattern from kg-mock-invoke.ts: registry → scenarios →
 * handler factories → installable mock wrapper.
 *
 * Usage in Playwright tests:
 *
 *   // Inject via page.addInitScript before navigating:
 *   import { calendarInvokeRegistry } from './calendar-mock-invoke';
 *
 *   page.addInitScript(() => {
 *     window.__CALENDAR_MOCK_SCENARIO__ = 'authorized';
 *   });
 *
 * Then in the app's Tauri mock layer, read __CALENDAR_MOCK_SCENARIO__
 * and delegate to the appropriate registry.
 */

// ── Types ────────────────────────────────────────────────────────────────

/** A handler function for a mocked Tauri command. */
export type CalendarInvokeHandler = (args: Record<string, unknown>) => unknown;

/** Registry mapping command names to handler functions. */
export class CalendarInvokeRegistry {
  private handlers = new Map<string, CalendarInvokeHandler>();
  private callLog: { command: string; args: Record<string, unknown> }[] = [];

  register(command: string, handler: CalendarInvokeHandler): void {
    this.handlers.set(command, handler);
  }

  clear(): void {
    this.handlers.clear();
    this.callLog = [];
  }

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

  getCalls(): { command: string; args: Record<string, unknown> }[] {
    return [...this.callLog];
  }

  getCallsFor(command: string): { command: string; args: Record<string, unknown> }[] {
    return this.callLog.filter((c) => c.command === command);
  }

  resetCallLog(): void {
    this.callLog = [];
  }
}

// ── Fixture Data ─────────────────────────────────────────────────────────

export const CALENDAR_FIXTURE_SETTINGS_DEFAULT = {
  metadata_pull_enabled: true,
  auto_record_enabled: false,
  provider: 'apple' as const,
  lookahead_window_minutes: 60,
  start_grace_window_minutes: 5,
  end_grace_window_minutes: 5,
  selected_apple_calendar_identifiers: [] as string[],
  show_calendar_status: true,
  show_next_meeting_banner: true,
};

export const CALENDAR_FIXTURE_CANDIDATE_TEAM_STANDUP = {
  candidates: [
    {
      id: 'evt-1',
      occurrence_key: 'evt-1|1752069600',
      title: 'Team Standup',
      start: '2026-07-08T14:00:00Z',
      end: '2026-07-08T14:30:00Z',
      calendar_id: 'cal-work',
      meeting_link: 'https://meet.google.com/abc-defg-hij',
      is_cancelled: false,
      category: 'Timed',
      response_status: 'Accepted',
      eligible: true,
      ineligibility_reason: null,
    },
    {
      id: 'evt-2',
      occurrence_key: 'evt-2|1752073200',
      title: 'Sprint Review',
      start: '2026-07-08T15:00:00Z',
      end: '2026-07-08T16:00:00Z',
      calendar_id: 'cal-work',
      meeting_link: 'https://zoom.us/j/456',
      is_cancelled: false,
      category: 'Timed',
      response_status: 'Accepted',
      eligible: true,
      ineligibility_reason: null,
    },
  ],
};

export const CALENDAR_FIXTURE_CANDIDATES_EMPTY = {
  candidates: [] as unknown[],
};

export const CALENDAR_FIXTURE_HEALTH_HEALTHY = {
  provider: 'apple',
  platform_supported: true,
  permission_granted: true,
  permission_status: 'authorized',
  event_count: 3,
  error: null,
};

export const CALENDAR_FIXTURE_HEALTH_DENIED = {
  provider: 'apple',
  platform_supported: true,
  permission_granted: false,
  permission_status: 'denied',
  event_count: null,
  error: null,
};

export const CALENDAR_FIXTURE_HEALTH_NOT_DETERMINED = {
  provider: 'apple',
  platform_supported: true,
  permission_granted: false,
  permission_status: 'not_determined',
  event_count: null,
  error: null,
};

export const CALENDAR_FIXTURE_HEALTH_UNSUPPORTED = {
  provider: 'apple',
  platform_supported: false,
  permission_granted: false,
  permission_status: 'unsupported_platform',
  event_count: null,
  error: 'Apple Calendar EventKit is only available on macOS.',
};

export const CALENDAR_FIXTURE_HEALTH_RESTRICTED = {
  provider: 'apple',
  platform_supported: true,
  permission_granted: false,
  permission_status: 'restricted',
  event_count: null,
  error: null,
};

export const CALENDAR_FIXTURE_SELECTED_CALENDARS: string[] = ['cal-work', 'cal-personal'];

// ── Scenarios ────────────────────────────────────────────────────────────

/**
 * Pre-built scenario identifiers for calendar QA flows.
 */
export enum CalendarMockScenario {
  /** Full access: permission authorized, healthy provider, upcoming candidates exist. */
  AUTHORIZED = 'authorized',
  /** Permission denied: provider healthy but access denied. */
  DENIED = 'denied',
  /** Permission not determined: Apple Calendar access not yet requested. */
  NOT_DETERMINED = 'not-determined',
  /** Unsupported platform: not running on macOS. */
  UNSUPPORTED = 'unsupported',
  /** Permission restricted: managed device / parental controls. */
  RESTRICTED = 'restricted',
  /** No upcoming candidates: authorized but nothing in the lookahead window. */
  NO_CANDIDATES = 'no-candidates',
  /** Scheduler triggered: simulates a recording_started event via mocked listen. */
  AUTO_RECORD_STARTED = 'auto-record-started',
}

// ── Handler Factories ────────────────────────────────────────────────────

function constHandler<T>(value: T): CalendarInvokeHandler {
  return () => value;
}

function errorHandler(message: string): CalendarInvokeHandler {
  return () => {
    throw new Error(message);
  };
}

// ── Scenario Builder ─────────────────────────────────────────────────────

/**
 * Build a registry pre-populated for the given scenario.
 */
export function buildCalendarScenarioRegistry(
  scenario: CalendarMockScenario
): CalendarInvokeRegistry {
  const registry = new CalendarInvokeRegistry();

  // Common save handler: return what was saved
  registry.register('save_calendar_settings', (args) => {
    return { ...CALENDAR_FIXTURE_SETTINGS_DEFAULT, ...(args.settings as object ?? {}) };
  });

  switch (scenario) {
    case CalendarMockScenario.AUTHORIZED:
      registry.register('get_calendar_settings', constHandler(CALENDAR_FIXTURE_SETTINGS_DEFAULT));
      registry.register('get_calendar_permission_status', constHandler('authorized'));
      registry.register('request_calendar_permission', constHandler('authorized'));
      registry.register('get_calendar_provider_health', constHandler(CALENDAR_FIXTURE_HEALTH_HEALTHY));
      registry.register('get_upcoming_calendar_candidates', constHandler(CALENDAR_FIXTURE_CANDIDATE_TEAM_STANDUP));
      registry.register('get_selected_calendars', constHandler(CALENDAR_FIXTURE_SELECTED_CALENDARS));
      break;

    case CalendarMockScenario.DENIED:
      registry.register('get_calendar_settings', constHandler({
        ...CALENDAR_FIXTURE_SETTINGS_DEFAULT,
        auto_record_enabled: false,
      }));
      registry.register('get_calendar_permission_status', constHandler('denied'));
      registry.register('request_calendar_permission', errorHandler('Calendar access was denied by the user'));
      registry.register('get_calendar_provider_health', constHandler(CALENDAR_FIXTURE_HEALTH_DENIED));
      registry.register('get_upcoming_calendar_candidates', constHandler(CALENDAR_FIXTURE_CANDIDATES_EMPTY));
      registry.register('get_selected_calendars', constHandler([] as string[]));
      break;

    case CalendarMockScenario.NOT_DETERMINED:
      registry.register('get_calendar_settings', constHandler(CALENDAR_FIXTURE_SETTINGS_DEFAULT));
      registry.register('get_calendar_permission_status', constHandler('not_determined'));
      registry.register('request_calendar_permission', constHandler('authorized'));
      registry.register('get_calendar_provider_health', constHandler(CALENDAR_FIXTURE_HEALTH_NOT_DETERMINED));
      registry.register('get_upcoming_calendar_candidates', constHandler(CALENDAR_FIXTURE_CANDIDATES_EMPTY));
      registry.register('get_selected_calendars', constHandler([] as string[]));
      break;

    case CalendarMockScenario.UNSUPPORTED:
      registry.register('get_calendar_settings', constHandler({
        ...CALENDAR_FIXTURE_SETTINGS_DEFAULT,
        metadata_pull_enabled: false,
        auto_record_enabled: false,
      }));
      registry.register('get_calendar_permission_status', constHandler('unsupported_platform'));
      registry.register('request_calendar_permission', errorHandler('Calendar access is not supported on this platform'));
      registry.register('get_calendar_provider_health', constHandler(CALENDAR_FIXTURE_HEALTH_UNSUPPORTED));
      registry.register('get_upcoming_calendar_candidates', constHandler(CALENDAR_FIXTURE_CANDIDATES_EMPTY));
      registry.register('get_selected_calendars', constHandler([] as string[]));
      break;

    case CalendarMockScenario.RESTRICTED:
      registry.register('get_calendar_settings', constHandler({
        ...CALENDAR_FIXTURE_SETTINGS_DEFAULT,
        metadata_pull_enabled: false,
        auto_record_enabled: false,
      }));
      registry.register('get_calendar_permission_status', constHandler('restricted'));
      registry.register('request_calendar_permission', errorHandler('Calendar access is restricted on this device'));
      registry.register('get_calendar_provider_health', constHandler(CALENDAR_FIXTURE_HEALTH_RESTRICTED));
      registry.register('get_upcoming_calendar_candidates', constHandler(CALENDAR_FIXTURE_CANDIDATES_EMPTY));
      registry.register('get_selected_calendars', constHandler([] as string[]));
      break;

    case CalendarMockScenario.NO_CANDIDATES:
      registry.register('get_calendar_settings', constHandler(CALENDAR_FIXTURE_SETTINGS_DEFAULT));
      registry.register('get_calendar_permission_status', constHandler('authorized'));
      registry.register('request_calendar_permission', constHandler('authorized'));
      registry.register('get_calendar_provider_health', constHandler(CALENDAR_FIXTURE_HEALTH_HEALTHY));
      registry.register('get_upcoming_calendar_candidates', constHandler(CALENDAR_FIXTURE_CANDIDATES_EMPTY));
      registry.register('get_selected_calendars', constHandler(CALENDAR_FIXTURE_SELECTED_CALENDARS));
      break;

    case CalendarMockScenario.AUTO_RECORD_STARTED:
      registry.register('get_calendar_settings', constHandler({
        ...CALENDAR_FIXTURE_SETTINGS_DEFAULT,
        auto_record_enabled: true,
      }));
      registry.register('get_calendar_permission_status', constHandler('authorized'));
      registry.register('request_calendar_permission', constHandler('authorized'));
      registry.register('get_calendar_provider_health', constHandler(CALENDAR_FIXTURE_HEALTH_HEALTHY));
      registry.register('get_upcoming_calendar_candidates', constHandler(CALENDAR_FIXTURE_CANDIDATE_TEAM_STANDUP));
      registry.register('get_selected_calendars', constHandler(CALENDAR_FIXTURE_SELECTED_CALENDARS));
      break;

    default: {
      const _exhaustive: never = scenario;
      throw new Error(`Unknown calendar scenario: ${_exhaustive}`);
    }
  }

  return registry;
}

// ── Installable Mock ─────────────────────────────────────────────────────

export interface InstallableCalendarMock {
  registry: CalendarInvokeRegistry;
  scenario: CalendarMockScenario;
  install: () => void;
  uninstall: () => void;
  reset: () => void;
  getCalls: () => { command: string; args: Record<string, unknown> }[];
}

export function createCalendarMock(
  scenario: CalendarMockScenario
): InstallableCalendarMock {
  const registry = buildCalendarScenarioRegistry(scenario);
  let installed = false;

  return {
    registry,
    scenario,

    install: () => {
      if (installed) return;
      installed = true;
    },

    uninstall: () => {
      installed = false;
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

// ── Playwright Injection Script ──────────────────────────────────────────

/**
 * Serialize a scenario registry as a JavaScript object for injection
 * into the browser via `page.addInitScript`. Returns a string of JS
 * that, when evaluated, patches `window.__TAURI_INTERNALS__` or the
 * module-level mock to return fixture data.
 *
 * In Playwright, call this BEFORE `page.goto()`:
 *
 *   await page.addInitScript({ content: injectCalendarMock(CalendarMockScenario.AUTHORIZED) });
 */
export function injectCalendarMock(scenario: CalendarMockScenario): string {
  const registry = buildCalendarScenarioRegistry(scenario);

  // Build a handler map that resolve() can use
  const handlerCases: string[] = [];
  for (const scenarioEnum of Object.values(CalendarMockScenario)) {
    const r = buildCalendarScenarioRegistry(scenarioEnum);
    const entries: string[] = [];
    r.resolve('__internal', {}); // trigger any init side effects
    // We rebuild fresh for serialization
  }

  // Simpler approach: build a hardcoded dispatch for each scenario
  const scenarioHandlers: Record<string, string> = {};

  for (const s of Object.values(CalendarMockScenario)) {
    const r = buildCalendarScenarioRegistry(s);
    // Extract the handlers and encode them
    const handlerMap = (r as unknown as { handlers: Map<string, CalendarInvokeHandler> }).handlers;
    const pairs: string[] = [];
    for (const [cmd, handler] of handlerMap) {
      // Try invoking with empty args to get the return value
      try {
        const result = handler({});
        pairs.push(`'${cmd}': ${JSON.stringify(result)}`);
      } catch {
        pairs.push(`'${cmd}': { __error__: 'Error from mock' }`);
      }
    }
    scenarioHandlers[s] = `{${pairs.join(',')}}`;
  }

  const scenarioMap = Object.entries(scenarioHandlers)
    .map(([k, v]) => `'${k}': ${v}`)
    .join(',');

  return `
(function() {
  var SCENARIOS = {${scenarioMap}};
  var scenario = '${scenario}';
  var data = SCENARIOS[scenario] || {};

  // Patch window.__TAURI_INTERNALS__.invoke before any app code runs
  Object.defineProperty(window, '__TAURI_INTERNALS__', {
    value: {
      invoke: function(cmd, args) {
        args = args || {};
        window.__CALENDAR_MOCK_CALLS__ = window.__CALENDAR_MOCK_CALLS__ || [];
        window.__CALENDAR_MOCK_CALLS__.push({ command: cmd, args: args });

        if (cmd === 'save_calendar_settings') {
          data['save_calendar_settings'] = (args.settings || {}).auto_record_enabled !== undefined
            ? args.settings
            : data['save_calendar_settings'];
          return Promise.resolve(args.settings || data['save_calendar_settings']);
        }

        if (data.hasOwnProperty(cmd)) {
          var result = data[cmd];
          if (result && result.__error__) {
            return Promise.reject(new Error(result.__error__));
          }
          return Promise.resolve(result);
        }

        return Promise.reject(new Error('No mock handler registered for command: ' + cmd));
      }
    },
    writable: true,
    configurable: true
  });

  window.__CALENDAR_MOCK_SCENARIO__ = '${scenario}';
  window.__CALENDAR_MOCK_CALLS__ = [];
})();
`;
}

// ── Fixture Triggers ─────────────────────────────────────────────────────

/**
 * Fixture trigger identifiers for calendar QA flows.
 */
export const CALENDAR_FIXTURE_TRIGGERS = {
  SETTINGS_OPEN: 'settings-open',
  TOGGLE_METADATA_PULL: 'toggle-metadata-pull',
  TOGGLE_AUTO_RECORD: 'toggle-auto-record',
  REQUEST_PERMISSION: 'request-permission',
  CHECK_PERMISSION_DENIED: 'check-permission-denied',
  CHECK_UNSUPPORTED_PLATFORM: 'check-unsupported-platform',
  SCHEDULER_AUTO_START: 'scheduler-auto-start',
} as const;

export type CalendarFixtureTrigger = (typeof CALENDAR_FIXTURE_TRIGGERS)[keyof typeof CALENDAR_FIXTURE_TRIGGERS];

/**
 * Map a fixture trigger to the scenario it should activate.
 */
export const CALENDAR_TRIGGER_TO_SCENARIO: Record<CalendarFixtureTrigger, CalendarMockScenario> = {
  [CALENDAR_FIXTURE_TRIGGERS.SETTINGS_OPEN]: CalendarMockScenario.AUTHORIZED,
  [CALENDAR_FIXTURE_TRIGGERS.TOGGLE_METADATA_PULL]: CalendarMockScenario.AUTHORIZED,
  [CALENDAR_FIXTURE_TRIGGERS.TOGGLE_AUTO_RECORD]: CalendarMockScenario.AUTHORIZED,
  [CALENDAR_FIXTURE_TRIGGERS.REQUEST_PERMISSION]: CalendarMockScenario.NOT_DETERMINED,
  [CALENDAR_FIXTURE_TRIGGERS.CHECK_PERMISSION_DENIED]: CalendarMockScenario.DENIED,
  [CALENDAR_FIXTURE_TRIGGERS.CHECK_UNSUPPORTED_PLATFORM]: CalendarMockScenario.UNSUPPORTED,
  [CALENDAR_FIXTURE_TRIGGERS.SCHEDULER_AUTO_START]: CalendarMockScenario.AUTO_RECORD_STARTED,
};
