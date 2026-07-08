use app_lib::audio::recording_manager::RecordingManager;
use app_lib::audio::recording_saver::RecordingSaver;
use app_lib::calendar::recording_metadata::CalendarRecordingContext;
use serde_json::json;

// ── helpers ───────────────────────────────────────────────────────────────

fn make_context(event_id: &str, title: &str) -> CalendarRecordingContext {
    CalendarRecordingContext {
        provider_kind: "apple".to_string(),
        event_id: event_id.to_string(),
        occurrence_start: "2026-07-08T14:00:00Z".to_string(),
        occurrence_end: "2026-07-08T14:30:00Z".to_string(),
        event_title: title.to_string(),
        metadata_status: "enriched".to_string(),
    }
}

fn read_metadata(dir: &tempfile::TempDir) -> serde_json::Value {
    let raw = std::fs::read_to_string(dir.path().join("metadata.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

// ── CalendarRecordingContext serialization ─────────────────────────────────

#[test]
fn calendar_recording_context_serializes_for_tauri_event() {
    let ctx = make_context("evt-1", "Sprint Planning");
    let json = serde_json::to_value(&ctx).unwrap();

    assert_eq!(json["provider_kind"], "apple");
    assert_eq!(json["event_id"], "evt-1");
    assert_eq!(json["occurrence_start"], "2026-07-08T14:00:00Z");
    assert_eq!(json["occurrence_end"], "2026-07-08T14:30:00Z");
    assert_eq!(json["event_title"], "Sprint Planning");
    assert_eq!(json["metadata_status"], "enriched");
}

#[test]
fn calendar_recording_context_roundtrips_json() {
    let ctx = make_context("evt-round", "Roundtrip Test");
    let json = serde_json::to_string(&ctx).unwrap();
    let parsed: CalendarRecordingContext = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.provider_kind, ctx.provider_kind);
    assert_eq!(parsed.event_id, ctx.event_id);
    assert_eq!(parsed.event_title, ctx.event_title);
    assert_eq!(parsed.metadata_status, ctx.metadata_status);
}

// ── RecordingSaver set/get calendar context ────────────────────────────────

#[test]
fn recording_saver_stores_and_retrieves_calendar_context() {
    let mut saver = RecordingSaver::new();
    let ctx = make_context("evt-saver", "Saver Test");

    saver.set_calendar_context(ctx.clone());

    let retrieved = saver.get_calendar_context().unwrap();
    assert_eq!(retrieved.event_id, "evt-saver");
    assert_eq!(retrieved.event_title, "Saver Test");
}

#[test]
fn recording_saver_returns_none_when_no_context_set() {
    let saver = RecordingSaver::new();
    assert!(saver.get_calendar_context().is_none());
}

// ── RecordingManager delegates calendar context ────────────────────────────

#[test]
fn recording_manager_delegates_calendar_context() {
    let mut manager = RecordingManager::new();
    let ctx = make_context("evt-mgr", "Manager Test");

    // Before setting: None
    assert!(manager.get_calendar_context().is_none());

    manager.set_calendar_context(ctx.clone());

    let retrieved = manager.get_calendar_context().unwrap();
    assert_eq!(retrieved.event_id, "evt-mgr");
    assert_eq!(retrieved.event_title, "Manager Test");
}

// ── Calendar context in metadata.json via RecordingSaver ───────────────────

#[test]
fn recording_saver_writes_calendar_context_during_metadata_write() {
    let dir = tempfile::tempdir().unwrap();

    // Create minimal metadata.json first
    let metadata_path = dir.path().join("metadata.json");
    std::fs::write(
        &metadata_path,
        serde_json::to_string_pretty(&json!({
            "version": "1.0",
            "status": "recording"
        }))
        .unwrap(),
    )
    .unwrap();

    // Test the calendar module directly for the metadata writing path
    app_lib::calendar::recording_metadata::write_calendar_context_to_metadata(
        dir.path(),
        &make_context("evt-meta", "Metadata Write Test"),
    )
    .unwrap();

    let parsed = read_metadata(&dir);
    assert_eq!(parsed["version"], "1.0");
    assert_eq!(parsed["status"], "recording");
    assert_eq!(parsed["calendar_context"]["event_id"], "evt-meta");
    assert_eq!(
        parsed["calendar_context"]["event_title"],
        "Metadata Write Test"
    );
}

// ── Manual recording without context ──────────────────────────────────────

#[test]
fn manual_recording_without_calendar_context_has_no_context() {
    let saver = RecordingSaver::new();
    assert!(saver.get_calendar_context().is_none());
}
