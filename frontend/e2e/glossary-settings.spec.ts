/**
 * Glossary Settings E2E Playwright Tests (Task 7)
 *
 * Verifies the Settings > Glossary flow:
 * - Opens settings page, selects the Glossary tab
 * - Adds an entry (person / code_name / acronym) with aliases, definition, and references
 * - Autosaves glossary and confirms success toast
 * - Autosyncs to KG with mocked Tauri invoke and confirms success toast
 * - Tests validation failure for blank term
 * - Tests edit mode with inline editing and Save button
 * - Tests navigation away auto-save
 *
 * All Tauri invoke calls are mocked via addInitScript — no real Rust
 * backend, KG server, or personal glossary file is used.
 */

import { test, expect } from '@playwright/test';

// ── Mock data ────────────────────────────────────────────────────────────

const EMPTY_GLOSSARY = {
  version: 1,
  entries: [],
};

const SAVED_GLOSSARY = {
  version: 1,
  entries: [
    {
      id: 'entry-alice',
      term: 'Alice',
      kind: 'person',
      pronunciation: '',
      aliases: ['A. Smith'],
      definition: 'Team lead',
      notes: '',
      references: ['https://example.com/alice'],
    },
  ],
};

const SYNC_SUCCESS = {
  profile_id: 'local-1',
  synced: true,
  track_id: 'track-001',
  document_id: null,
  error: null,
  skipped_reason: null,
};

// ── Mock injection helper ────────────────────────────────────────────────

/**
 * Build the addInitScript content that patches Tauri's invoke function
 * before the app code loads. Returns a string of JavaScript.
 */
function buildMockScript(glossaryOverrides: Record<string, unknown>): string {
  const base: Record<string, unknown> = {
    // Glossary commands
    api_get_glossary: EMPTY_GLOSSARY,
    api_save_glossary: EMPTY_GLOSSARY,
    api_sync_glossary_to_knowledge_graph: SYNC_SUCCESS,

    // Onboarding bypass — required or the app redirects to onboarding and never shows Settings
    get_onboarding_status: { completed: true },

    // Calendar commands — CalendarSettings crashes if these return null
    get_calendar_settings: {
      metadata_pull_enabled: true,
      auto_record_enabled: false,
      provider: 'apple',
      lookahead_window_minutes: 60,
      start_grace_window_minutes: 5,
      end_grace_window_minutes: 5,
      selected_apple_calendar_identifiers: [],
      show_calendar_status: true,
      show_next_meeting_banner: true,
    },
    get_calendar_permission_status: 'authorized',
    get_calendar_provider_health: {
      provider: 'apple',
      platform_supported: true,
      permission_granted: true,
      permission_status: 'authorized',
      event_count: 3,
      error: null,
    },
    get_upcoming_calendar_candidates: { candidates: [] },
    get_selected_calendars: ['cal-work'],
    request_calendar_permission: 'authorized',

    // Settings page base commands (copied from calendar-settings.spec.ts)
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
    ...glossaryOverrides,
  };

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

      if (cmd === 'api_save_glossary') {
        // Return the glossary that was passed in, mimicking Rust normalization
        return Promise.resolve(args.glossary || EMPTY_GLOSSARY);
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

  window.__TAURI_EVENT_LISTENERS__ = {};
  window.__TAURI_INTERNALS__.listen = function(event, handler) {
    window.__TAURI_EVENT_LISTENERS__[event] = window.__TAURI_EVENT_LISTENERS__[event] || [];
    window.__TAURI_EVENT_LISTENERS__[event].push(handler);
    return Promise.resolve(function() {
      var idx = window.__TAURI_EVENT_LISTENERS__[event].indexOf(handler);
      if (idx >= 0) window.__TAURI_EVENT_LISTENERS__[event].splice(idx, 1);
    });
  };

  window.__EMIT_TAURI_EVENT__ = function(event, payload) {
    var listeners = window.__TAURI_EVENT_LISTENERS__[event] || [];
    listeners.forEach(function(fn) { fn({ payload: payload }); });
  };

  window.__GLOSSARY_MOCK_CALLS__ = calls;
  window.__GLOSSARY_MOCK_SCENARIO__ = 'default';
})();
`;
}

// ── Test fixture ─────────────────────────────────────────────────────────

test.describe('Glossary Settings', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript({ content: buildMockScript({}) });
  });

  // ── Settings page renders Glossary tab ───────────────────────────────

  test('renders Glossary tab in Settings', async ({ page }) => {
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Click the Glossary tab
    await page.getByRole('tab', { name: /Glossary/i }).click();

    // Verify the Glossary heading renders
    await expect(page.getByText('Glossary').first()).toBeVisible({ timeout: 10_000 });

    // Verify the descriptive text
    await expect(page.getByText(/Define terms, people, projects, and acronyms/)).toBeVisible({ timeout: 10_000 });

    // Verify empty-state message
    await expect(page.getByText('No Glossary Entries')).toBeVisible({ timeout: 10_000 });
  });

  // ── Happy path: add entry, save, sync ──────────────────────────────────

  test('adds an entry via inline edit, saves, and syncs to KG', async ({ page }) => {
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Select Glossary tab
    await page.getByRole('tab', { name: /Glossary/i }).click();

    // Click Add First Entry — enters edit mode with a blank row
    await page.getByRole('button', { name: /Add First Entry/i }).click();

    // Verify Save button appears (edit mode)
    await expect(page.getByRole('button', { name: /Save/i })).toBeVisible({ timeout: 10_000 });

    // Fill the blank row inline
    await page.locator('input[id^="edit-term-"]').fill('Alice');
    await page.locator('button[id^="edit-kind-"]').click();
    await page.getByRole('option', { name: 'person' }).click();
    await page.locator('input[id^="edit-aliases-"]').fill('A. Smith');
    await page.locator('textarea[id^="edit-definition-"]').fill('Team lead');
    await page.locator('textarea[id^="edit-references-"]').fill('https://example.com/alice');

    // Click Save to persist
    await page.getByRole('button', { name: /Save/i }).click();

    // Verify the entry appears in the list
    await expect(page.getByText('Alice', { exact: true })).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('person').first()).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('A. Smith')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('Team lead').first()).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('https://example.com/alice')).toBeVisible({ timeout: 10_000 });

    await expect(page.getByText('Glossary saved')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('Glossary synced to Knowledge Graph')).toBeVisible({ timeout: 10_000 });
  });

  // ── Validation failure: blank term ─────────────────────────────────────

  test('shows validation error when term is blank in inline edit', async ({ page }) => {
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Select Glossary tab
    await page.getByRole('tab', { name: /Glossary/i }).click();

    // Click Add First Entry — enters edit mode with a blank row
    await page.getByRole('button', { name: /Add First Entry/i }).click();

    // Leave term blank and click Save
    await page.getByRole('button', { name: /Save/i }).click();

    // Validation error toast should appear
    await expect(page.getByText('Cannot save: one or more entries have a blank term')).toBeVisible({ timeout: 10_000 });

    // Still in edit mode (Save button still visible)
    await expect(page.getByRole('button', { name: /Save/i })).toBeVisible({ timeout: 10_000 });

    // Cancel to return to empty state
    await page.getByRole('button', { name: /Cancel/i }).click();

    // Empty state should still be present
    await expect(page.getByText('No Glossary Entries')).toBeVisible({ timeout: 10_000 });
  });

  // ── Edit mode: inline editing and Save ────────────────────────────────

  test('enters edit mode, edits inline, and saves', async ({ page }) => {
    // Pre-populate glossary with one entry
    await page.addInitScript({
      content: buildMockScript({
        api_get_glossary: SAVED_GLOSSARY,
      }),
    });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Select Glossary tab
    await page.getByRole('tab', { name: /Glossary/i }).click();

    // Verify entry renders
    await expect(page.getByText('Alice', { exact: true })).toBeVisible({ timeout: 10_000 });

    // Click Edit button to enter edit mode
    await page.getByRole('button', { name: /Edit$/i }).click();

    // Verify Save button appears
    await expect(page.getByRole('button', { name: /Save/i })).toBeVisible({ timeout: 10_000 });

    // Edit the term inline
    await page.locator('input[id^="edit-term-"]').fill('Alice Smith');

    // Click Save
    await page.getByRole('button', { name: /Save/i }).click();

    // Verify updated term
    await expect(page.getByText('Alice Smith')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('Glossary saved')).toBeVisible({ timeout: 10_000 });
  });

  // ── Delete entry ─────────────────────────────────────────────────────

  test('deletes an existing entry', async ({ page }) => {
    // Pre-populate glossary with one entry
    await page.addInitScript({
      content: buildMockScript({
        api_get_glossary: SAVED_GLOSSARY,
      }),
    });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Select Glossary tab
    await page.getByRole('tab', { name: /Glossary/i }).click();

    // Verify entry renders
    await expect(page.getByText('Alice', { exact: true })).toBeVisible({ timeout: 10_000 });

    // Click delete
    await page.getByRole('button', { name: /Delete Alice/i }).click();

    // Confirm delete in dialog
    await page.getByRole('button', { name: /Delete$/i }).click();

    // Delete enters edit mode; save to persist removal
    await page.getByRole('button', { name: /Save/i }).click();

    // Verify empty state returns
    await expect(page.getByText('No Glossary Entries')).toBeVisible({ timeout: 10_000 });
  });

  test('adds blank entry at top when glossary already has entries', async ({ page }) => {
    await page.addInitScript({
      content: buildMockScript({
        api_get_glossary: SAVED_GLOSSARY,
      }),
    });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    await page.getByRole('tab', { name: /Glossary/i }).click();

    await expect(page.getByText('Alice', { exact: true })).toBeVisible({ timeout: 10_000 });

    await page.getByRole('button', { name: /Add Entry/i }).click();

    await expect(page.getByRole('button', { name: /Save/i })).toBeVisible({ timeout: 10_000 });

    const termInputs = page.locator('input[id^="edit-term-"]');
    await expect(termInputs).toHaveCount(2, { timeout: 10_000 });
    await expect(termInputs.nth(0)).toHaveValue('');
    await expect(termInputs.nth(1)).toHaveValue('Alice');
  });

  // ── Navigation away auto-save ─────────────────────────────────────────

  test('auto-saves pending edits when navigating away from glossary tab', async ({ page }) => {
    // Pre-populate glossary with one entry
    await page.addInitScript({
      content: buildMockScript({
        api_get_glossary: SAVED_GLOSSARY,
      }),
    });

    await page.goto('/settings');
    await page.waitForLoadState('networkidle');

    // Select Glossary tab
    await page.getByRole('tab', { name: /Glossary/i }).click();

    // Verify entry renders
    await expect(page.getByText('Alice', { exact: true })).toBeVisible({ timeout: 10_000 });

    // Enter edit mode
    await page.getByRole('button', { name: /Edit$/i }).click();

    // Edit the term inline
    await page.locator('input[id^="edit-term-"]').fill('Alice Smith');

    // Navigate to another tab
    await page.getByRole('tab', { name: /General/i }).click();

    await expect(page.getByText('Glossary saved')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByRole('tab', { name: /General/i })).toHaveAttribute('data-state', 'active');
  });

});
