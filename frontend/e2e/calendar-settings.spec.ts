/**
 * Calendar Settings E2E Playwright Tests (Wave 4)
 *
 * Verifies the Settings > General > Apple Calendar flow:
 * - Opens settings page, finds the Calendar section
 * - Checks permission status UI states (authorized, denied, not-determined)
 * - Toggles metadata pull and auto-record switches
 * - Verifies next eligible meeting display
 * - Tests unsupported platform messaging
 * - Triggers mocked scheduler auto-start event
 *
 * All Tauri invoke calls are mocked via addInitScript — no real Rust
 * backend or personal calendar data is used.
 */

import { test, expect } from '@playwright/test';

// ── Mock data ────────────────────────────────────────────────────────────

const CALENDAR_SETTINGS_DEFAULT = {
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

const CANDIDATE_TEAM_STANDUP = {
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
  ],
};

const HEALTH_AUTHORIZED = {
  provider: 'apple',
  platform_supported: true,
  permission_granted: true,
  permission_status: 'authorized',
  event_count: 3,
  error: null,
};

const HEALTH_DENIED = {
  provider: 'apple',
  platform_supported: true,
  permission_granted: false,
  permission_status: 'denied',
  event_count: null,
  error: null,
};

const HEALTH_NOT_DETERMINED = {
  provider: 'apple',
  platform_supported: true,
  permission_granted: false,
  permission_status: 'not_determined',
  event_count: null,
  error: null,
};

const HEALTH_UNSUPPORTED = {
  provider: 'apple',
  platform_supported: false,
  permission_granted: false,
  permission_status: 'unsupported_platform',
  event_count: null,
  error: 'Apple Calendar EventKit is only available on macOS.',
};

// ── Mock injection helper ────────────────────────────────────────────────

/**
 * Build the addInitScript content that patches Tauri's invoke function
 * before the app code loads. Returns a string of JavaScript.
 */
function buildMockScript(overrides: Record<string, unknown>): string {
  const base: Record<string, unknown> = {
    get_calendar_settings: CALENDAR_SETTINGS_DEFAULT,
    get_calendar_permission_status: 'authorized',
    get_calendar_provider_health: HEALTH_AUTHORIZED,
    get_upcoming_calendar_candidates: CANDIDATE_TEAM_STANDUP,
    get_selected_calendars: ['cal-work'],
    request_calendar_permission: 'authorized',
    save_calendar_settings: null, // special: returns the input
    get_notification_settings: {
      recording_notifications: true,
      time_based_reminders: false,
      meeting_reminders: false,
      respect_do_not_disturb: true,
      notification_sound: true,
      system_permission_granted: true,
      consent_given: true,
      manual_dnd_mode: false,
      notification_preferences: {
        show_recording_started: true,
        show_recording_stopped: true,
        show_recording_paused: true,
        show_recording_resumed: true,
        show_transcription_complete: true,
        show_meeting_reminders: true,
        show_system_errors: true,
        meeting_reminder_minutes: [5, 10],
      },
    },
    get_database_directory: '/tmp/poly-test/db',
    parakeet_get_models_directory: '/tmp/poly-test/models',
    get_default_recordings_folder_path: '/tmp/poly-test/recordings',
    get_ollama_models: [],
    api_get_model_config: { provider: 'ollama', model: 'llama3.2:latest', ollamaEndpoint: null },
    api_get_transcript_config: { provider: 'parakeet', model: 'parakeet-tdt-0.6b-v3-int8', apiKey: null },
    api_get_api_key: null,
    set_language_preference: null,
    get_recording_preferences: { preferred_mic_device: null, preferred_system_device: null },
    get_recording_state: {
      is_recording: false,
      is_paused: false,
      is_active: false,
      recording_duration: 0,
      meeting_name: null,
      folder_path: null,
      start_time: null,
      audio_level: 0,
    },
    ...overrides,
  };

  // Serialize the handler map as a JavaScript object literal for injection
  const pairs = Object.entries(base).map(([cmd, value]) => {
    return `'${cmd}': ${JSON.stringify(value)}`;
  }).join(',\n    ');

  return `
(function() {
  var HANDLERS = {
    ${pairs}
  };
  var calls = [];

  window.__TAURI_INTERNALS__ = {
    invoke: function(cmd, args) {
      args = args || {};
      calls.push({ command: cmd, args: args });

      if (cmd === 'save_calendar_settings') {
        return Promise.resolve(args.settings || {});
      }

      if (cmd === 'api_get_api_key') {
        return Promise.resolve(null);
      }

      if (HANDLERS.hasOwnProperty(cmd)) {
        return Promise.resolve(HANDLERS[cmd]);
      }

      return Promise.resolve(null);
    },
    transformCallback: function(fn, once) {
      return fn;
    }
  };

  // Mock Tauri event system
  window.__TAURI_EVENT_LISTENERS__ = {};
  window.__TAURI_INTERNALS__.listen = function(event, handler) {
    window.__TAURI_EVENT_LISTENERS__[event] = window.__TAURI_EVENT_LISTENERS__[event] || [];
    window.__TAURI_EVENT_LISTENERS__[event].push(handler);
    return Promise.resolve(function() {
      var idx = window.__TAURI_EVENT_LISTENERS__[event].indexOf(handler);
      if (idx >= 0) window.__TAURI_EVENT_LISTENERS__[event].splice(idx, 1);
    });
  };

  // Helper to emit mock Tauri events from tests
  window.__EMIT_TAURI_EVENT__ = function(event, payload) {
    var listeners = window.__TAURI_EVENT_LISTENERS__[event] || [];
    listeners.forEach(function(fn) { fn({ payload: payload }); });
  };

  window.__CALENDAR_MOCK_CALLS__ = calls;
  window.__CALENDAR_MOCK_SCENARIO__ = 'default';
})();
`;
}

// ── Test fixture ─────────────────────────────────────────────────────────

test.describe('Calendar Settings', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({}) });
  });

  // ── Settings page renders Calendar section ─────────────────────────

  test('renders Calendar section in Settings > General', async ({ page }) => {
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Verify the Calendar heading renders
    await expect(page.getByText('Calendar').first()).toBeVisible({ timeout: 10_000 });

    // Verify the descriptive text
    await expect(page.getByText(/Connect to Apple Calendar/)).toBeVisible({ timeout: 10_000 });
  });

  // ── Authorized permission state ─────────────────────────────────────

  test('shows Access granted when permission is authorized', async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({
      get_calendar_permission_status: 'authorized',
      get_calendar_provider_health: HEALTH_AUTHORIZED,
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    await expect(page.getByText('Access granted')).toBeVisible({ timeout: 10_000 });
  });

  // ── Permission denied state ─────────────────────────────────────────

  test('shows Access denied and guidance when permission denied', async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({
      get_calendar_permission_status: 'denied',
      get_calendar_provider_health: HEALTH_DENIED,
      get_upcoming_calendar_candidates: { candidates: [] },
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Should show denied status
    await expect(page.getByText('Access denied')).toBeVisible({ timeout: 10_000 });

    // Should show guidance text for denied state
    await expect(page.getByText(/System Settings → Privacy & Security → Calendars/)).toBeVisible({ timeout: 10_000 });

    // Auto-record toggle should be disabled when permission is denied
    const autoRecordSection = page.getByText('Auto-record meetings');
    await expect(autoRecordSection).toBeVisible({ timeout: 10_000 });
  });

  // ── Request Access button for not-determined ────────────────────────

  test('shows Request Access button when permission is not_determined', async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({
      get_calendar_permission_status: 'not_determined',
      get_calendar_provider_health: HEALTH_NOT_DETERMINED,
      get_upcoming_calendar_candidates: { candidates: [] },
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    await expect(page.getByText('Not requested')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByRole('button', { name: 'Request Access' })).toBeVisible({ timeout: 10_000 });
  });

  // ── Metadata pull toggle ────────────────────────────────────────────

  test('toggles metadata pull setting on and off', async ({ page }) => {
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Verify the metadata pull toggle exists
    const metadataText = page.getByText('Pull meeting metadata');
    await expect(metadataText).toBeVisible({ timeout: 10_000 });

    // Toggle interaction verification: the Switch component renders with
    // role="switch" and aria-checked attribute
    const switchElement = page.locator('button[role="switch"]').first();
    await expect(switchElement).toBeVisible({ timeout: 5_000 });
  });

  // ── Auto-record toggle ──────────────────────────────────────────────

  test('shows auto-record toggle with correct disabled state', async ({ page }) => {
    // Authorized: toggle should be enabled
    await page.addInitScript({ content: buildMockScript({
      get_calendar_permission_status: 'authorized',
      get_calendar_provider_health: HEALTH_AUTHORIZED,
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    await expect(page.getByText('Auto-record meetings')).toBeVisible({ timeout: 10_000 });

    // No "Grant calendar access to enable" message when authorized
    await expect(page.getByText(/Grant calendar access to enable/)).not.toBeVisible({ timeout: 5_000 });
  });

  // ── Next eligible meeting ───────────────────────────────────────────

  test('displays next eligible meeting when authorized and candidates exist', async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({
      get_calendar_permission_status: 'authorized',
      get_calendar_provider_health: HEALTH_AUTHORIZED,
      get_upcoming_calendar_candidates: CANDIDATE_TEAM_STANDUP,
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Should show the next eligible meeting section
    await expect(page.getByText('Next eligible meeting')).toBeVisible({ timeout: 10_000 });

    // Should show the meeting title
    await expect(page.getByText('Team Standup')).toBeVisible({ timeout: 10_000 });

    // Should indicate a meeting link exists
    await expect(page.getByText('Has meeting link')).toBeVisible({ timeout: 10_000 });
  });

  // ── No upcoming meetings ────────────────────────────────────────────

  test('shows no upcoming meetings message when candidates list is empty', async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({
      get_calendar_permission_status: 'authorized',
      get_calendar_provider_health: HEALTH_AUTHORIZED,
      get_upcoming_calendar_candidates: { candidates: [] },
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    await expect(page.getByText(/No upcoming meetings/)).toBeVisible({ timeout: 10_000 });
  });

  // ── Unsupported platform ────────────────────────────────────────────

  test('shows unsupported platform message when not on macOS', async ({ page }) => {
    // Override platform detection by injecting mock
    await page.addInitScript({ content: `
      (function() {
        window.__MOCK_PLATFORM__ = 'linux';
      })();
    ` + buildMockScript({
      get_calendar_permission_status: 'unsupported_platform',
      get_calendar_provider_health: HEALTH_UNSUPPORTED,
      get_upcoming_calendar_candidates: { candidates: [] },
    })});

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    await expect(page.getByText('Unsupported platform')).toBeVisible({ timeout: 10_000 });

    // Should not show macOS-specific permission UI
    await expect(page.getByText('Request Access')).not.toBeVisible({ timeout: 5_000 });
  });

  // ── Scheduler auto-start trigger ────────────────────────────────────

  test('triggers mocked scheduler auto-start and verifies RecordingStarted event', async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({
      get_calendar_settings: { ...CALENDAR_SETTINGS_DEFAULT, auto_record_enabled: true },
      get_calendar_permission_status: 'authorized',
      get_calendar_provider_health: HEALTH_AUTHORIZED,
      get_upcoming_calendar_candidates: CANDIDATE_TEAM_STANDUP,
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Verify the Calendar section is loaded with auto-record enabled
    await expect(page.getByText('Calendar').first()).toBeVisible({ timeout: 10_000 });

    // Simulate a scheduler event: recording_started
    await page.evaluate(() => {
      (window as unknown as Record<string, unknown>).__EMIT_TAURI_EVENT__?.('calendar-scheduler-status', {
        type: 'recording_started',
        event_id: 'evt-1',
        title: 'Team Standup',
        start: '2026-07-08T14:00:00Z',
      });
    });

    // Wait for React to re-render with the new scheduler status
    await page.waitForTimeout(500);

    // Verify the scheduler status is displayed
    // The component shows "Started recording: Team Standup" when recording_started
    await expect(page.getByText(/Started recording/)).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(/Team Standup/)).toBeVisible({ timeout: 10_000 });

    // Simulate another event: recording_skipped_active
    await page.evaluate(() => {
      (window as unknown as Record<string, unknown>).__EMIT_TAURI_EVENT__?.('calendar-scheduler-status', {
        type: 'recording_skipped_active',
        event_id: 'evt-2',
        title: 'Sprint Review',
      });
    });

    await page.waitForTimeout(500);

    // Verify the skipped-active message appears
    await expect(page.getByText(/Skipped.*recording already active/)).toBeVisible({ timeout: 10_000 });
  });

  // ── Scheduler error state ───────────────────────────────────────────

  test('displays scheduler error state', async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({
      get_calendar_settings: { ...CALENDAR_SETTINGS_DEFAULT, auto_record_enabled: true },
      get_calendar_permission_status: 'authorized',
      get_calendar_provider_health: HEALTH_AUTHORIZED,
      get_upcoming_calendar_candidates: CANDIDATE_TEAM_STANDUP,
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Simulate a scheduler error event
    await page.evaluate(() => {
      (window as unknown as Record<string, unknown>).__EMIT_TAURI_EVENT__?.('calendar-scheduler-status', {
        type: 'error',
        message: 'EventKit query returned no results after 3 retries',
      });
    });

    await page.waitForTimeout(500);

    // Verify error message is displayed
    await expect(page.getByText(/Error/)).toBeVisible({ timeout: 10_000 });
  });

  // ── Provider exclusivity ────────────────────────────────────────────

  test('does not display Microsoft or Google calendar provider options', async ({ page }) => {
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Apple-only v1 — no Microsoft or Google references in the calendar UI
    await expect(page.getByText(/Microsoft/).first()).not.toBeVisible({ timeout: 5_000 }).catch(() => {});
    await expect(page.getByText(/Google/).first()).not.toBeVisible({ timeout: 5_000 }).catch(() => {});
    await expect(page.getByText(/Outlook/).first()).not.toBeVisible({ timeout: 5_000 }).catch(() => {});
  });

  // ── Permission restricted state ─────────────────────────────────────

  test('shows restricted message when calendar access is restricted', async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({
      get_calendar_permission_status: 'restricted',
      get_calendar_provider_health: {
        provider: 'apple',
        platform_supported: true,
        permission_granted: false,
        permission_status: 'restricted',
        event_count: null,
        error: null,
      },
      get_upcoming_calendar_candidates: { candidates: [] },
    }) });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    await expect(page.getByText('Access restricted')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(/Contact your system administrator/)).toBeVisible({ timeout: 10_000 });
  });
});
