use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

const METADATA_FILE: &str = "metadata.json";
const CALENDAR_CONTEXT_FIELD: &str = "calendar_context";

/// Safe calendar fields permitted in recording-folder metadata.json.
///
/// Primitives only — no attendees, full body, meeting URL, or raw provider
/// payloads are written to the folder.  Those enrichments belong in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarRecordingContext {
    /// Provider kind (e.g. "apple", "google", "microsoft", "fake").
    pub provider_kind: String,
    /// Opaque event identifier from the provider.
    pub event_id: String,
    /// Occurrence start time in RFC 3339.
    pub occurrence_start: String,
    /// Occurrence end time in RFC 3339.
    pub occurrence_end: String,
    /// Human-readable event title.
    pub event_title: String,
    /// Enrichment status: "enriched", "partial", or "missing".
    pub metadata_status: String,
}

/// Write safe calendar context into the recording folder's metadata.json.
///
/// Uses atomic temp-file-and-rename and preserves every existing top-level
/// field that is not `calendar_context`.  If metadata.json does not exist yet
/// a new object containing only `calendar_context` is created.
pub fn write_calendar_context_to_metadata(
    folder: &Path,
    context: &CalendarRecordingContext,
) -> Result<()> {
    let metadata_path = folder.join(METADATA_FILE);
    let temp_path = folder.join(format!(
        ".metadata.json.calendar.{}.tmp",
        uuid::Uuid::new_v4()
    ));

    // Read existing metadata, preserving unknown fields
    let mut value: Value = if metadata_path.exists() {
        let raw = std::fs::read_to_string(&metadata_path)
            .with_context(|| format!("Failed to read {}", metadata_path.display()))?;
        serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse {}", metadata_path.display()))?
    } else {
        Value::Object(serde_json::Map::new())
    };

    if !value.is_object() {
        anyhow::bail!(
            "metadata.json root value must be a JSON object, got {}",
            if value.is_array() { "array" } else { "scalar" }
        );
    }

    let object = value.as_object_mut().expect("checked as object above");

    let calendar_value = serde_json::json!({
        "provider_kind": context.provider_kind,
        "event_id": context.event_id,
        "occurrence_start": context.occurrence_start,
        "occurrence_end": context.occurrence_end,
        "event_title": context.event_title,
        "metadata_status": context.metadata_status,
    });

    object.insert(CALENDAR_CONTEXT_FIELD.to_string(), calendar_value);

    let json_string =
        serde_json::to_string_pretty(&value).context("Failed to serialize metadata.json")?;
    std::fs::write(&temp_path, &json_string)
        .with_context(|| format!("Failed to write {}", temp_path.display()))?;
    std::fs::rename(&temp_path, &metadata_path).with_context(|| {
        format!(
            "Failed to replace {} with {}",
            metadata_path.display(),
            temp_path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup(dir: &tempfile::TempDir, json: Value) {
        std::fs::write(
            dir.path().join(METADATA_FILE),
            serde_json::to_string_pretty(&json).unwrap(),
        )
        .unwrap();
    }

    fn read(dir: &tempfile::TempDir) -> Value {
        let raw = std::fs::read_to_string(dir.path().join(METADATA_FILE)).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    fn ctx() -> CalendarRecordingContext {
        CalendarRecordingContext {
            provider_kind: "apple".to_string(),
            event_id: "evt-1".to_string(),
            occurrence_start: "2026-07-08T14:00:00Z".to_string(),
            occurrence_end: "2026-07-08T14:30:00Z".to_string(),
            event_title: "Sprint Planning".to_string(),
            metadata_status: "enriched".to_string(),
        }
    }

    #[test]
    fn writes_calendar_context_to_existing_metadata() {
        let dir = tempfile::tempdir().unwrap();
        setup(&dir, json!({"version": "1.0", "meeting_name": "Test"}));

        write_calendar_context_to_metadata(dir.path(), &ctx()).unwrap();

        let parsed = read(&dir);
        assert_eq!(parsed["version"], "1.0");
        assert_eq!(parsed["meeting_name"], "Test");
        let cc = &parsed["calendar_context"];
        assert_eq!(cc["provider_kind"], "apple");
        assert_eq!(cc["event_id"], "evt-1");
        assert_eq!(cc["event_title"], "Sprint Planning");
    }

    #[test]
    fn creates_metadata_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        // no metadata.json on disk

        write_calendar_context_to_metadata(dir.path(), &ctx()).unwrap();

        let parsed = read(&dir);
        assert_eq!(parsed["calendar_context"]["event_id"], "evt-1");
    }

    #[test]
    fn preserves_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        setup(
            &dir,
            json!({"version": "1.0", "custom_field": "keep-me", "status": "recording"}),
        );

        write_calendar_context_to_metadata(dir.path(), &ctx()).unwrap();

        let parsed = read(&dir);
        assert_eq!(parsed["version"], "1.0");
        assert_eq!(parsed["custom_field"], "keep-me");
        assert_eq!(parsed["status"], "recording");
        assert_eq!(parsed["calendar_context"]["event_id"], "evt-1");
    }

    #[test]
    fn overwrites_existing_calendar_context() {
        let dir = tempfile::tempdir().unwrap();
        setup(
            &dir,
            json!({
                "version": "1.0",
                "calendar_context": {
                    "provider_kind": "google",
                    "event_id": "old-id",
                    "occurrence_start": "2000-01-01T00:00:00Z",
                    "occurrence_end": "2000-01-01T01:00:00Z",
                    "event_title": "Old",
                    "metadata_status": "partial"
                }
            }),
        );

        write_calendar_context_to_metadata(dir.path(), &ctx()).unwrap();

        let parsed = read(&dir);
        assert_eq!(parsed["calendar_context"]["provider_kind"], "apple");
        assert_eq!(parsed["calendar_context"]["event_id"], "evt-1");
    }

    #[test]
    fn no_attendees_or_body_in_output() {
        let dir = tempfile::tempdir().unwrap();
        setup(&dir, json!({"version": "1.0"}));

        write_calendar_context_to_metadata(dir.path(), &ctx()).unwrap();

        let raw = std::fs::read_to_string(dir.path().join(METADATA_FILE)).unwrap();
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        let cc = &parsed["calendar_context"];

        assert!(cc.get("attendees").is_none());
        assert!(cc.get("body").is_none());
        assert!(cc.get("meeting_link").is_none());
        assert!(cc.get("organizer").is_none());
        assert!(cc.get("location").is_none());
    }

    #[test]
    fn rejects_non_object_metadata() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(METADATA_FILE), "[]").unwrap();

        let err = write_calendar_context_to_metadata(dir.path(), &ctx()).unwrap_err();
        assert!(err.to_string().contains("must be a JSON object"));
    }
}
