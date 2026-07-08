use app_lib::calendar::recording_metadata::{
    write_calendar_context_to_metadata, CalendarRecordingContext,
};
use serde_json::json;

fn setup_metadata_json(dir: &tempfile::TempDir, json: serde_json::Value) {
    std::fs::write(
        dir.path().join("metadata.json"),
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();
}

fn read_metadata(dir: &tempfile::TempDir) -> serde_json::Value {
    let raw = std::fs::read_to_string(dir.path().join("metadata.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn calendar_context() -> CalendarRecordingContext {
    CalendarRecordingContext {
        provider_kind: "apple".to_string(),
        event_id: "event-abc-123".to_string(),
        occurrence_start: "2026-07-08T14:00:00Z".to_string(),
        occurrence_end: "2026-07-08T14:30:00Z".to_string(),
        event_title: "Design Review".to_string(),
        metadata_status: "enriched".to_string(),
    }
}

#[test]
fn calendar_context_is_written_to_metadata_json() {
    let dir = tempfile::tempdir().unwrap();
    setup_metadata_json(
        &dir,
        json!({
            "version": "1.0",
            "meeting_name": "Test Meeting"
        }),
    );

    write_calendar_context_to_metadata(dir.path(), &calendar_context()).unwrap();

    let parsed = read_metadata(&dir);
    let cc = &parsed["calendar_context"];
    assert_eq!(cc["provider_kind"], "apple");
    assert_eq!(cc["event_id"], "event-abc-123");
    assert_eq!(cc["occurrence_start"], "2026-07-08T14:00:00Z");
    assert_eq!(cc["occurrence_end"], "2026-07-08T14:30:00Z");
    assert_eq!(cc["event_title"], "Design Review");
    assert_eq!(cc["metadata_status"], "enriched");
}

#[test]
fn calendar_context_preserves_existing_metadata_fields() {
    let dir = tempfile::tempdir().unwrap();
    setup_metadata_json(
        &dir,
        json!({
            "version": "1.0",
            "meeting_id": "meeting-456",
            "meeting_name": "Standup",
            "devices": {
                "microphone": "Built-in Mic",
                "system_audio": "BlackHole 2ch"
            },
            "status": "recording"
        }),
    );

    write_calendar_context_to_metadata(dir.path(), &calendar_context()).unwrap();

    let parsed = read_metadata(&dir);
    assert_eq!(parsed["version"], "1.0");
    assert_eq!(parsed["meeting_id"], "meeting-456");
    assert_eq!(parsed["meeting_name"], "Standup");
    assert_eq!(parsed["devices"]["microphone"], "Built-in Mic");
    assert_eq!(parsed["devices"]["system_audio"], "BlackHole 2ch");
    assert_eq!(parsed["status"], "recording");
    // Calendar context is present
    assert_eq!(parsed["calendar_context"]["event_id"], "event-abc-123");
}

#[test]
fn calendar_context_updates_existing_calendar_context_field() {
    let dir = tempfile::tempdir().unwrap();
    setup_metadata_json(
        &dir,
        json!({
            "version": "1.0",
            "calendar_context": {
                "provider_kind": "google",
                "event_id": "old-event-id",
                "occurrence_start": "2026-01-01T00:00:00Z",
                "occurrence_end": "2026-01-01T01:00:00Z",
                "event_title": "Old Title",
                "metadata_status": "partial"
            }
        }),
    );

    write_calendar_context_to_metadata(dir.path(), &calendar_context()).unwrap();

    let parsed = read_metadata(&dir);
    let cc = &parsed["calendar_context"];
    // All fields updated to new values
    assert_eq!(cc["provider_kind"], "apple");
    assert_eq!(cc["event_id"], "event-abc-123");
    assert_eq!(cc["event_title"], "Design Review");
    assert_eq!(cc["metadata_status"], "enriched");
    // Non-calendar-context fields preserved
    assert_eq!(parsed["version"], "1.0");
}

#[test]
fn calendar_context_does_not_write_attendees_or_body() {
    let dir = tempfile::tempdir().unwrap();
    setup_metadata_json(&dir, json!({"version": "1.0"}));

    write_calendar_context_to_metadata(dir.path(), &calendar_context()).unwrap();

    let raw = std::fs::read_to_string(dir.path().join("metadata.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();

    // calendar_context is present
    assert!(parsed.get("calendar_context").is_some());

    let cc = &parsed["calendar_context"];
    assert!(
        cc.get("attendees").is_none(),
        "attendees leaked into metadata"
    );
    assert!(cc.get("body").is_none(), "body leaked into metadata");
    assert!(
        cc.get("meeting_url").is_none(),
        "meeting_url leaked into metadata"
    );
    assert!(
        cc.get("meeting_link").is_none(),
        "meeting_link leaked into metadata"
    );
    assert!(
        cc.get("organizer").is_none(),
        "organizer leaked into metadata"
    );
    assert!(
        cc.get("location").is_none(),
        "location leaked into metadata"
    );
    assert!(
        cc.get("raw_payload").is_none(),
        "raw_payload leaked into metadata"
    );

    // Verify raw JSON string also doesn't contain attendee-like data
    let lower = raw.to_lowercase();
    assert!(
        !lower.contains("attendee"),
        "attendee string found in raw metadata"
    );
}

#[test]
fn calendar_context_reads_valid_json_when_metadata_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    // metadata.json exists with empty object
    setup_metadata_json(&dir, json!({}));

    write_calendar_context_to_metadata(dir.path(), &calendar_context()).unwrap();

    let parsed = read_metadata(&dir);
    assert_eq!(parsed["calendar_context"]["event_id"], "event-abc-123");
}

#[test]
fn calendar_context_creates_metadata_when_none_exists() {
    let dir = tempfile::tempdir().unwrap();
    // No metadata.json exists

    write_calendar_context_to_metadata(dir.path(), &calendar_context()).unwrap();

    let parsed = read_metadata(&dir);
    assert_eq!(parsed["calendar_context"]["event_id"], "event-abc-123");
    assert_eq!(parsed["calendar_context"]["event_title"], "Design Review");
}
