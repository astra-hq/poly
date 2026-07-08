use crate::database::models::MeetingCalendarMetadata;
use chrono::Utc;
use sqlx::{Error as SqlxError, SqliteConnection, SqlitePool};
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct CalendarMetadataInput {
    pub provider_kind: String,
    pub provider_event_id: String,
    pub occurrence_start_utc: String,
    pub occurrence_end_utc: String,
    pub event_title: Option<String>,
    pub organizer_email: Option<String>,
    pub organizer_display_name: Option<String>,
    pub attendees_json: Option<String>,
    pub invite_body: Option<String>,
    pub meeting_url: Option<String>,
    pub location: Option<String>,
    pub source_provider_kind: Option<String>,
    pub source_calendar_id: Option<String>,
}

pub struct CalendarMetadataRepository;

impl CalendarMetadataRepository {
    /// Insert calendar metadata within an existing transaction.
    /// Caller owns the transaction — commit or rollback.
    pub async fn insert_with_transaction(
        conn: &mut SqliteConnection,
        meeting_id: &str,
        input: &CalendarMetadataInput,
    ) -> Result<(), SqlxError> {
        let now = Utc::now().to_rfc3339();

        let result = sqlx::query(
            "INSERT INTO meeting_calendar_metadata (
                meeting_id, provider_kind, provider_event_id,
                occurrence_start_utc, occurrence_end_utc,
                event_title, organizer_email, organizer_display_name,
                attendees_json, invite_body, meeting_url, location,
                source_provider_kind, source_calendar_id,
                metadata_status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'fetched', ?, ?)",
        )
        .bind(meeting_id)
        .bind(&input.provider_kind)
        .bind(&input.provider_event_id)
        .bind(&input.occurrence_start_utc)
        .bind(&input.occurrence_end_utc)
        .bind(&input.event_title)
        .bind(&input.organizer_email)
        .bind(&input.organizer_display_name)
        .bind(&input.attendees_json)
        .bind(&input.invite_body)
        .bind(&input.meeting_url)
        .bind(&input.location)
        .bind(&input.source_provider_kind)
        .bind(&input.source_calendar_id)
        .bind(&now)
        .bind(&now)
        .execute(&mut *conn)
        .await;

        match result {
            Ok(_) => {
                info!(
                    "Saved calendar metadata for meeting {} (provider: {}, event: {})",
                    meeting_id, input.provider_kind, input.provider_event_id
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to save calendar metadata for meeting {}: {}",
                    meeting_id, e
                );
                Err(e)
            }
        }
    }

    /// Retrieve calendar metadata by meeting ID.
    pub async fn get_by_meeting_id(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Option<MeetingCalendarMetadata>, SqlxError> {
        sqlx::query_as::<_, MeetingCalendarMetadata>(
            "SELECT * FROM meeting_calendar_metadata WHERE meeting_id = ?",
        )
        .bind(meeting_id)
        .fetch_optional(pool)
        .await
    }
}
