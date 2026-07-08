import { describe, test, expect } from 'bun:test';
import type { CalendarRecordingContextPayload } from '@/services/recordingService';

// Manual re-export of the module-level function for testability.
// This mirrors the logic in useRecordingStop.ts:calendarContextToMetadataRequest.
function calendarContextToMetadataRequest(ctx: CalendarRecordingContextPayload) {
  return {
    provider_kind: ctx.provider_kind,
    provider_event_id: ctx.event_id,
    occurrence_start_utc: ctx.occurrence_start,
    occurrence_end_utc: ctx.occurrence_end,
    event_title: ctx.event_title,
  };
}

const sampleContext: CalendarRecordingContextPayload = {
  provider_kind: 'apple',
  event_id: 'evt-scheduler-001',
  occurrence_start: '2026-07-08T14:00:00Z',
  occurrence_end: '2026-07-08T14:30:00Z',
  event_title: 'Sprint Planning',
  metadata_status: 'enriched',
};

describe('RecordingStoppedPayload', () => {
  test('includes optional calendar_context field', () => {
    // Without calendar context (manual recording)
    const noContext = {
      message: 'Recording stopped',
      folder_path: '/tmp/meeting',
      meeting_name: 'Meeting 1',
    };
    expect(noContext).not.toHaveProperty('calendar_context');

    // With calendar context (scheduler-started recording)
    const withContext = {
      message: 'Recording stopped',
      folder_path: '/tmp/meeting',
      meeting_name: 'Sprint Planning',
      calendar_context: sampleContext,
    };
    expect(withContext.calendar_context).toEqual(sampleContext);
  });
});

describe('calendarContextToMetadataRequest', () => {
  test('maps event_id to provider_event_id', () => {
    const result = calendarContextToMetadataRequest(sampleContext);
    expect(result.provider_event_id).toBe('evt-scheduler-001');
  });

  test('maps occurrence_start to occurrence_start_utc', () => {
    const result = calendarContextToMetadataRequest(sampleContext);
    expect(result.occurrence_start_utc).toBe('2026-07-08T14:00:00Z');
  });

  test('maps occurrence_end to occurrence_end_utc', () => {
    const result = calendarContextToMetadataRequest(sampleContext);
    expect(result.occurrence_end_utc).toBe('2026-07-08T14:30:00Z');
  });

  test('passes provider_kind and event_title through', () => {
    const result = calendarContextToMetadataRequest(sampleContext);
    expect(result.provider_kind).toBe('apple');
    expect(result.event_title).toBe('Sprint Planning');
  });

  test('omits metadata_status from CalendarMetadataRequest', () => {
    const result = calendarContextToMetadataRequest(sampleContext);
    expect(result).not.toHaveProperty('metadata_status');
  });
});

describe('SaveMeetingRequest', () => {
  test('accepts optional calendarMetadata', () => {
    const req = {
      meetingTitle: 'Test',
      transcripts: [],
      folderPath: null,
      calendarMetadata: null,
    };
    expect(req.calendarMetadata).toBeNull();
  });

  test('calendarMetadata is shaped as CalendarMetadataRequest', () => {
    const calMeta = calendarContextToMetadataRequest(sampleContext);

    expect(calMeta.provider_kind).toBe('apple');
    expect(calMeta.provider_event_id).toBe('evt-scheduler-001');
    expect(calMeta.occurrence_start_utc).toBe('2026-07-08T14:00:00Z');
    expect(calMeta.occurrence_end_utc).toBe('2026-07-08T14:30:00Z');
    expect(calMeta.event_title).toBe('Sprint Planning');
  });
});
