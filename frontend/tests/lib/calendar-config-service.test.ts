import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';

type InvokeCall = {
  readonly command: string;
  readonly args: Record<string, unknown>;
};

const DEFAULT_CALENDAR_SETTINGS = {
  metadata_pull_enabled: true,
  auto_record_enabled: false,
  provider: 'apple',
  lookahead_window_minutes: 60,
  start_grace_window_minutes: 5,
  end_grace_window_minutes: 5,
  selected_apple_calendar_identifiers: [],
  show_calendar_status: true,
  show_next_meeting_banner: true,
} as const;

const CUSTOM_CALENDAR_SETTINGS = {
  ...DEFAULT_CALENDAR_SETTINGS,
  auto_record_enabled: true,
  lookahead_window_minutes: 120,
  start_grace_window_minutes: 10,
  end_grace_window_minutes: 15,
  selected_apple_calendar_identifiers: ['calendar://work'],
  show_next_meeting_banner: false,
} as const;

const calls: InvokeCall[] = [];

const mockInvoke = mock((command: string, args?: Record<string, unknown>) => {
  calls.push({ command, args: args ?? {} });

  if (command === 'get_calendar_settings') {
    return Promise.resolve(DEFAULT_CALENDAR_SETTINGS);
  }

  if (command === 'save_calendar_settings') {
    return Promise.resolve(CUSTOM_CALENDAR_SETTINGS);
  }

  if (command === 'skip_calendar_occurrence') {
    return Promise.resolve(undefined);
  }

  return Promise.reject(new Error(`No mock handler registered for command: ${command}`));
});

mock.module('@tauri-apps/api/core', () => ({
  invoke: (...args: Parameters<typeof mockInvoke>) => mockInvoke(...args),
}));

const { ConfigService } = await import('../../src/services/configService');

describe('ConfigService calendar settings', () => {
  beforeEach(() => {
    calls.length = 0;
  });

  afterEach(() => {
    calls.length = 0;
  });

  test('getCalendarSettings calls get_calendar_settings with no args', async () => {
    const service = new ConfigService();

    const result = await service.getCalendarSettings();

    expect(result).toEqual(DEFAULT_CALENDAR_SETTINGS);
    expect(calls).toEqual([{ command: 'get_calendar_settings', args: {} }]);
  });

  test('saveCalendarSettings calls save_calendar_settings with settings arg', async () => {
    const service = new ConfigService();

    const result = await service.saveCalendarSettings(CUSTOM_CALENDAR_SETTINGS);

    expect(result).toEqual(CUSTOM_CALENDAR_SETTINGS);
    expect(calls).toEqual([
      {
        command: 'save_calendar_settings',
        args: { settings: CUSTOM_CALENDAR_SETTINGS },
      },
    ]);
  });

  test('calendar settings expose Apple as the only v1 provider literal', () => {
    expect(DEFAULT_CALENDAR_SETTINGS.provider).toBe('apple');
    expect(CUSTOM_CALENDAR_SETTINGS.provider).toBe('apple');
  });

  test('skipCalendarOccurrence calls skip_calendar_occurrence with event and start', async () => {
    const service = new ConfigService();

    await service.skipCalendarOccurrence('evt-1', '2026-07-08T14:00:00Z');

    expect(calls).toEqual([
      {
        command: 'skip_calendar_occurrence',
        args: {
          eventId: 'evt-1',
          occurrenceStart: '2026-07-08T14:00:00Z',
        },
      },
    ]);
  });
});
