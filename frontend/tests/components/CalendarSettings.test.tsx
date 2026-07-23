import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

// ── Types for mocked context ───────────────────────────────────────────────

type CalendarPermissionStatus =
  | 'not_determined'
  | 'restricted'
  | 'denied'
  | 'authorized'
  | 'full_access'
  | 'write_only'
  | 'unsupported_platform'
  | 'unknown';

interface CalendarConfig {
  readonly metadata_pull_enabled: boolean;
  readonly auto_record_enabled: boolean;
  readonly provider: 'apple';
  readonly lookahead_window_minutes: number;
  readonly start_grace_window_minutes: number;
  readonly end_grace_window_minutes: number;
  readonly selected_apple_calendar_identifiers: readonly string[];
  readonly show_calendar_status: boolean;
  readonly show_next_meeting_banner: boolean;
}

interface CalendarProviderHealth {
  readonly provider: string;
  readonly platform_supported: boolean;
  readonly permission_granted: boolean;
  readonly permission_status: CalendarPermissionStatus;
  readonly event_count: number | null;
  readonly error: string | null;
}

interface CalendarCandidate {
  readonly id: string;
  readonly occurrence_key: string;
  readonly title: string;
  readonly start: string;
  readonly end: string;
  readonly calendar_id: string;
  readonly meeting_link: string | null;
  readonly is_cancelled: boolean;
  readonly category: string;
  readonly response_status: string;
  readonly eligible: boolean;
  readonly ineligibility_reason: string | null;
}

interface SchedulerStatusEvent {
  readonly type: string;
  readonly event_id?: string;
  readonly title?: string;
  readonly start?: string;
  readonly end?: string;
  readonly reason?: string;
  readonly message?: string;
}

// ── Mock invoke ────────────────────────────────────────────────────────────

const calls: { command: string; args: Record<string, unknown> }[] = [];

const DEFAULT_CALENDAR_SETTINGS: CalendarConfig = {
  metadata_pull_enabled: true,
  auto_record_enabled: false,
  provider: 'apple',
  lookahead_window_minutes: 60,
  start_grace_window_minutes: 5,
  end_grace_window_minutes: 5,
  selected_apple_calendar_identifiers: [],
  show_calendar_status: true,
  show_next_meeting_banner: true,
};

const mockInvoke = mock((command: string, args?: Record<string, unknown>) => {
  calls.push({ command, args: args ?? {} });

  if (command === 'get_calendar_settings') {
    return Promise.resolve(DEFAULT_CALENDAR_SETTINGS);
  }

  if (command === 'save_calendar_settings') {
    return Promise.resolve({ ...DEFAULT_CALENDAR_SETTINGS, ...(args?.settings as object ?? {}) });
  }

  if (command === 'get_calendar_permission_status') {
    return Promise.resolve('authorized');
  }

  if (command === 'request_calendar_permission') {
    return Promise.resolve('authorized');
  }

  if (command === 'get_calendar_provider_health') {
    return Promise.resolve({
      provider: 'apple',
      platform_supported: true,
      permission_granted: true,
      permission_status: 'authorized',
      event_count: 3,
      error: null,
    });
  }

  if (command === 'get_upcoming_calendar_candidates') {
    return Promise.resolve({
      candidates: [
        {
          id: 'evt-1',
          occurrence_key: 'evt-1|1234567890',
          title: 'Team Standup',
          start: '2026-07-08T14:00:00Z',
          end: '2026-07-08T14:30:00Z',
          calendar_id: 'cal-work',
          meeting_link: 'https://zoom.us/j/123',
          is_cancelled: false,
          category: 'Timed',
          response_status: 'Accepted',
          eligible: true,
          ineligibility_reason: null,
        },
      ],
    });
  }

  if (command === 'get_selected_calendars') {
    return Promise.resolve([]);
  }

  return Promise.reject(new Error(`No mock handler registered for command: ${command}`));
});

mock.module('@tauri-apps/api/core', () => ({
  invoke: (...args: Parameters<typeof mockInvoke>) => mockInvoke(...args),
}));

// ── Mock ConfigContext ───────────────────────────────────────────────────

let mockCalendarSettings: CalendarConfig = { ...DEFAULT_CALENDAR_SETTINGS };
let mockPermissionStatus: CalendarPermissionStatus = 'authorized';
let mockProviderHealth: CalendarProviderHealth | null = {
  provider: 'apple',
  platform_supported: true,
  permission_granted: true,
  permission_status: 'authorized',
  event_count: 3,
  error: null,
};
let mockUpcomingCandidates: CalendarCandidate[] = [
  {
    id: 'evt-1',
    occurrence_key: 'evt-1|1234567890',
    title: 'Team Standup',
    start: '2026-07-08T14:00:00Z',
    end: '2026-07-08T14:30:00Z',
    calendar_id: 'cal-work',
    meeting_link: 'https://zoom.us/j/123',
    is_cancelled: false,
    category: 'Timed',
    response_status: 'Accepted',
    eligible: true,
    ineligibility_reason: null,
  },
];
let mockSchedulerStatus: SchedulerStatusEvent | null = null;
let mockIsLoadingCalendar = false;
let mockPlatform: string = 'macos';
let mockAvailableCalendars: { readonly id: string; readonly title: string }[] = [
  { id: 'cal-work', title: 'Work' },
];

const mockSetCalendarSettings = mock((settings: CalendarConfig | ((prev: CalendarConfig) => CalendarConfig)) => {
  if (typeof settings === 'function') {
    mockCalendarSettings = settings(mockCalendarSettings);
  } else {
    mockCalendarSettings = settings;
  }
});

const mockUpdateCalendarSettings = mock(async (settings: CalendarConfig) => {
  mockCalendarSettings = settings;
  return settings;
});

const mockLoadCalendarStatus = mock(async () => {
  // no-op in mock
});

const mockRequestCalendarPermission = mock(async () => {
  mockPermissionStatus = 'authorized';
  return 'authorized';
});

const mockSkipCalendarOccurrence = mock(async () => undefined);

mock.module('@/contexts/ConfigContext', () => ({
  useConfig: () => ({
    calendarSettings: mockCalendarSettings,
    setCalendarSettings: mockSetCalendarSettings,
    updateCalendarSettings: mockUpdateCalendarSettings,
    calendarPermissionStatus: mockPermissionStatus,
    calendarProviderHealth: mockProviderHealth,
    upcomingCalendarCandidates: mockUpcomingCandidates,
    schedulerStatus: mockSchedulerStatus,
    isLoadingCalendar: mockIsLoadingCalendar,
    loadCalendarStatus: mockLoadCalendarStatus,
    requestCalendarPermission: mockRequestCalendarPermission,
    availableCalendars: mockAvailableCalendars,
    skipCalendarOccurrence: mockSkipCalendarOccurrence,
    platform: mockPlatform,
  }),
}));

// ── Mock usePlatform ───────────────────────────────────────────────────────

mock.module('@/hooks/usePlatform', () => ({
  usePlatform: () => mockPlatform,
}));

// ── Import component after mocks ─────────────────────────────────────────

const { CalendarSettings } = await import('../../src/components/CalendarSettings');

describe('CalendarSettings', () => {
  beforeEach(() => {
    calls.length = 0;
    mockCalendarSettings = { ...DEFAULT_CALENDAR_SETTINGS };
    mockPermissionStatus = 'authorized';
    mockProviderHealth = {
      provider: 'apple',
      platform_supported: true,
      permission_granted: true,
      permission_status: 'authorized',
      event_count: 3,
      error: null,
    };
    mockUpcomingCandidates = [
      {
        id: 'evt-1',
        occurrence_key: 'evt-1|1234567890',
        title: 'Team Standup',
        start: '2026-07-08T14:00:00Z',
        end: '2026-07-08T14:30:00Z',
        calendar_id: 'cal-work',
        meeting_link: 'https://zoom.us/j/123',
        is_cancelled: false,
        category: 'Timed',
        response_status: 'Accepted',
        eligible: true,
        ineligibility_reason: null,
      },
    ];
    mockSchedulerStatus = null;
    mockIsLoadingCalendar = false;
    mockPlatform = 'macos';
    mockAvailableCalendars = [{ id: 'cal-work', title: 'Work' }];

  });

  afterEach(() => {
    calls.length = 0;
  });

  test('renders calendar section title', () => {
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('Calendar');
  });

  test('hides granted permission status and shows revoke guidance when authorized', () => {
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).not.toContain('Access granted');
    expect(html).toContain('How to revoke calendar access');
  });

  test('shows permission denied state', () => {
    mockPermissionStatus = 'denied';
    mockProviderHealth = {
      provider: 'apple',
      platform_supported: true,
      permission_granted: false,
      permission_status: 'denied',
      event_count: null,
      error: null,
    };
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('Access denied');
    expect(html).not.toContain('Pull meeting metadata');
    expect(html).not.toContain('Auto-record meetings');
    expect(html).not.toContain('Next eligible meeting');
  });

  test('shows unsupported platform message on non-macOS', () => {
    mockPlatform = 'windows';
    mockPermissionStatus = 'unsupported_platform';
    mockProviderHealth = {
      provider: 'apple',
      platform_supported: false,
      permission_granted: false,
      permission_status: 'unsupported_platform',
      event_count: null,
      error: 'Apple Calendar EventKit is only available on macOS.',
    };
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('Unsupported platform');
    expect(html).toContain('macOS');
  });

  test('shows next eligible meeting', () => {
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('Team Standup');
  });

  test('shows metadata pull toggle', () => {
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('Pull meeting metadata');
  });

  test('shows auto-record toggle', () => {
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('Auto-record meetings');
  });

  test('does not show Microsoft or Google provider options', () => {
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).not.toContain('Microsoft');
    expect(html).not.toContain('Google');
    expect(html).not.toContain('Outlook');
    expect(html).not.toContain('Gmail');
  });

  test('shows request permission button when not determined', () => {
    mockPermissionStatus = 'not_determined';
    mockProviderHealth = {
      provider: 'apple',
      platform_supported: true,
      permission_granted: false,
      permission_status: 'not_determined',
      event_count: null,
      error: null,
    };
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('Request Access');
  });

  test('shows scheduler status when present', () => {
    mockSchedulerStatus = {
      type: 'recording_started',
      event_id: 'evt-1',
      title: 'Team Standup',
      start: '2026-07-08T14:00:00Z',
    };
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('Team Standup');
    expect(html).toContain('Recording');
  });

  test('shows no candidates message when upcoming list is empty', () => {
    mockUpcomingCandidates = [];
    const html = renderToStaticMarkup(<CalendarSettings />);
    expect(html).toContain('No upcoming meetings');
  });

  test('calendar config exposes Apple as only provider', () => {
    expect(mockCalendarSettings.provider).toBe('apple');
  });
});
