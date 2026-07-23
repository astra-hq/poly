import { describe, expect, test, mock, beforeEach, afterEach } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';

// ── Mock sonner toast ─────────────────────────────────────────────────────

const toastCalls: Array<{ type: string; title: string; description?: string }> = [];

const mockToast = {
  info: mock((title: string, opts?: { description?: string }) => {
    toastCalls.push({ type: 'info', title, description: opts?.description });
    return 'toast-id';
  }),
  success: mock((title: string, opts?: { description?: string }) => {
    toastCalls.push({ type: 'success', title, description: opts?.description });
    return 'toast-id';
  }),
  warning: mock((title: string, opts?: { description?: string }) => {
    toastCalls.push({ type: 'warning', title, description: opts?.description });
    return 'toast-id';
  }),
  error: mock((title: string, opts?: { description?: string }) => {
    toastCalls.push({ type: 'error', title, description: opts?.description });
    return 'toast-id';
  }),
  dismiss: mock(() => {}),
};

mock.module('sonner', () => ({
  toast: mockToast,
  Toaster: () => null,
  useSonner: () => ({ toasts: [] }),
}));

// ── Mock Tauri event listener ─────────────────────────────────────────────

const eventHandlers: Record<string, Array<(payload: unknown) => void>> = {};

const mockListen = mock(async (event: string, handler: (payload: unknown) => void) => {
  if (!eventHandlers[event]) {
    eventHandlers[event] = [];
  }
  eventHandlers[event].push(handler);
  return () => {
    const idx = eventHandlers[event].indexOf(handler);
    if (idx >= 0) eventHandlers[event].splice(idx, 1);
  };
});

mock.module('@tauri-apps/api/event', () => ({
  listen: mockListen,
}));

// ── Import component and notification helpers after mocks ─────────────────

const { CalendarSchedulerStatus } = await import('../../src/components/CalendarSchedulerStatus');
const {
  showCalendarSchedulerNotification,
  showCalendarAutoStartNotification,
  showCalendarSkipNotification,
} = await import('../../src/lib/recordingNotification');

// ── Test data ─────────────────────────────────────────────────────────────

const FORBIDDEN_NOTIFICATION_CONTENT = [
  'Board Strategy',
  'standup',
  'Standup',
  'meeting with',
  'zoom.us',
  'https://',
  'alice@',
  'bob@',
  'attendee',
  'organizer',
];

// ── Tests ─────────────────────────────────────────────────────────────────

describe('CalendarSchedulerStatus component', () => {
  test('renders idle state when no status', () => {
    const html = renderToStaticMarkup(<CalendarSchedulerStatus status={null} />);
    expect(html).toContain('Scheduler');
    expect(html).toContain('idle');
  });

  test('renders candidate found status', () => {
    const html = renderToStaticMarkup(
      <CalendarSchedulerStatus
        status={{
          type: 'candidate_found',
          event_id: 'evt-1',
          title: 'Standup',
          start: '2026-07-08T10:00:00Z',
          end: '2026-07-08T10:30:00Z',
        }}
      />
    );
    expect(html).toContain('Candidate found');
  });

  test('renders recording started status', () => {
    const html = renderToStaticMarkup(
      <CalendarSchedulerStatus
        status={{
          type: 'recording_started',
          event_id: 'evt-1',
          title: 'Standup',
          start: '2026-07-08T10:00:00Z',
        }}
      />
    );
    expect(html).toContain('Auto-record started');
  });

  test('renders recording skipped active status', () => {
    const html = renderToStaticMarkup(
      <CalendarSchedulerStatus
        status={{
          type: 'recording_skipped_active',
          event_id: 'evt-1',
          title: 'Standup',
        }}
      />
    );
    expect(html).toContain('Skipped');
    expect(html).toContain('active recording');
  });

  test('renders skipped status with reason', () => {
    const html = renderToStaticMarkup(
      <CalendarSchedulerStatus
        status={{
          type: 'skipped',
          reason: 'outside_grace_window',
        }}
      />
    );
    expect(html).toContain('Skipped');
    expect(html).toContain('outside_grace_window');
  });

  test('renders error status', () => {
    const html = renderToStaticMarkup(
      <CalendarSchedulerStatus
        status={{
          type: 'error',
          message: 'Provider error: test',
        }}
      />
    );
    expect(html).toContain('error');
    expect(html).toContain('Provider error: test');
  });

  test('renders stopped status', () => {
    const html = renderToStaticMarkup(
      <CalendarSchedulerStatus
        status={{
          type: 'stopped',
        }}
      />
    );
    expect(html).toContain('stopped');
  });
});

describe('Calendar scheduler notification content privacy', () => {
  beforeEach(() => {
    toastCalls.length = 0;
  });

  afterEach(() => {
    toastCalls.length = 0;
  });

  test('showCalendarAutoStartNotification uses generic body without meeting title', () => {
    showCalendarAutoStartNotification();

    expect(toastCalls.length).toBe(1);
    const call = toastCalls[0];
    expect(call.type).toBe('success');
    expect(call.title).toBe('Auto-Recording Started');
    expect(call.description).toBeDefined();

    // Must not contain sensitive calendar data
    const content = `${call.title} ${call.description}`;
    for (const forbidden of FORBIDDEN_NOTIFICATION_CONTENT) {
      expect(content.toLowerCase()).not.toContain(forbidden.toLowerCase());
    }
  });

  test('showCalendarSkipNotification uses generic reason without event details', () => {
    showCalendarSkipNotification('recording_active');

    expect(toastCalls.length).toBe(1);
    const call = toastCalls[0];
    expect(call.type).toBe('warning');
    expect(call.title).toBe('Auto-Record Skipped');
    expect(call.description).toBeDefined();

    const content = `${call.title} ${call.description}`;
    for (const forbidden of FORBIDDEN_NOTIFICATION_CONTENT) {
      expect(content.toLowerCase()).not.toContain(forbidden.toLowerCase());
    }
  });

  test('showCalendarSchedulerNotification for candidate_found does not show an in-app toast', () => {
    showCalendarSchedulerNotification('candidate_found', {
      event_id: 'evt-123',
      title: 'Secret Board Meeting',
      start: '2026-07-08T10:00:00Z',
      end: '2026-07-08T11:00:00Z',
    });

    expect(toastCalls.length).toBe(0);
  });

  test('showCalendarSchedulerNotification for error is privacy-safe', () => {
    showCalendarSchedulerNotification('error', {
      message: 'Provider error: something failed',
    });

    expect(toastCalls.length).toBe(1);
    const call = toastCalls[0];
    expect(call.type).toBe('error');
    expect(call.title).toBe('Scheduler Error');
  });
});

describe('Calendar scheduler notification non-blocking behavior', () => {
  beforeEach(() => {
    toastCalls.length = 0;
  });

  test('showCalendarAutoStartNotification does not throw when toast fails', () => {
    // Temporarily break toast
    const originalInfo = mockToast.info;
    mockToast.info = mock(() => {
      throw new Error('Toast system unavailable');
    });

    // Must not throw
    expect(() => showCalendarAutoStartNotification()).not.toThrow();

    mockToast.info = originalInfo;
  });

  test('showCalendarSkipNotification does not throw when toast fails', () => {
    const originalWarning = mockToast.warning;
    mockToast.warning = mock(() => {
      throw new Error('Toast system unavailable');
    });

    expect(() => showCalendarSkipNotification('permission_denied')).not.toThrow();

    mockToast.warning = originalWarning;
  });

  test('showCalendarSchedulerNotification does not throw on unknown status type', () => {
    expect(() =>
      showCalendarSchedulerNotification('unknown_type' as never, {})
    ).not.toThrow();
  });
});
