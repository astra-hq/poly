-- Add meeting_calendar_metadata table for calendar event metadata
-- linked to meetings. Supports Apple, Google, Microsoft, and fake providers.
-- Does NOT store OAuth tokens or secrets.
CREATE TABLE IF NOT EXISTS meeting_calendar_metadata (
    meeting_id                  TEXT PRIMARY KEY NOT NULL,
    provider_kind               TEXT NOT NULL,
    provider_event_id           TEXT NOT NULL,
    occurrence_start_utc        TEXT NOT NULL,
    occurrence_end_utc          TEXT NOT NULL,
    event_title                 TEXT,
    organizer_email             TEXT,
    organizer_display_name      TEXT,
    attendees_json              TEXT,
    invite_body                 TEXT,
    meeting_url                 TEXT,
    location                    TEXT,
    source_provider_kind        TEXT,
    source_calendar_id          TEXT,
    metadata_status             TEXT NOT NULL DEFAULT 'fetched',
    created_at                  TEXT NOT NULL,
    updated_at                  TEXT NOT NULL,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_meeting_calendar_metadata_provider
    ON meeting_calendar_metadata(provider_kind, provider_event_id);
