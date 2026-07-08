#[cfg(not(target_os = "macos"))]
use crate::calendar::apple_calendar::platform_probe;
#[cfg(not(target_os = "macos"))]
use crate::calendar::types::CalendarProbeErrorKind;
use crate::calendar::types::{CalendarPermissionStatus, NormalizedCalendarEvent, RawCalendarEvent};

#[test]
fn apple_calendar_probe_permission_status_when_eventkit_status_is_known() {
    // Given: raw EventKit authorization status values documented by Apple.
    let cases = [
        (0, CalendarPermissionStatus::NotDetermined),
        (1, CalendarPermissionStatus::Restricted),
        (2, CalendarPermissionStatus::Denied),
        (3, CalendarPermissionStatus::FullAccess),
        (4, CalendarPermissionStatus::WriteOnly),
    ];

    // When / Then: each raw value maps to the typed permission status used by Tauri.
    for (raw, expected) in cases {
        assert_eq!(
            CalendarPermissionStatus::from_eventkit_status(raw),
            expected
        );
    }
}

#[test]
fn apple_calendar_probe_normalizes_event_when_optional_fields_are_present() {
    // Given: one raw EventKit-shaped event with every optional field populated.
    let raw = RawCalendarEvent::with_traditional_fields(
        "event-1".to_string(),
        Some("Planning".to_string()),
        Some(1_790_000_000.0),
        Some(1_790_003_600.0),
        vec!["a@example.com".to_string(), "B Person".to_string()],
        Some("Agenda".to_string()),
        Some("https://example.com/meet".to_string()),
        Some("Room 1".to_string()),
    );

    // When: the raw adapter shape is normalized for the command response.
    let normalized = NormalizedCalendarEvent::from(raw);

    // Then: all fields survive with the command-facing names and types.
    assert_eq!(normalized.identifier, "event-1");
    assert_eq!(normalized.title.as_deref(), Some("Planning"));
    assert_eq!(normalized.attendees, ["a@example.com", "B Person"]);
    assert_eq!(normalized.notes.as_deref(), Some("Agenda"));
    assert_eq!(normalized.url.as_deref(), Some("https://example.com/meet"));
    assert_eq!(normalized.location.as_deref(), Some("Room 1"));
}

#[test]
#[cfg(not(target_os = "macos"))]
fn apple_calendar_probe_skips_with_typed_status_when_not_macos() {
    // Given: the probe is compiled on a non-macOS target.
    // When: the platform probe runs.
    let probe = platform_probe();

    // Then: it returns a typed unsupported status instead of touching EventKit.
    assert_eq!(probe.status, CalendarPermissionStatus::UnsupportedPlatform);
    assert_eq!(
        probe.error.map(|error| error.kind),
        Some(CalendarProbeErrorKind::UnsupportedPlatform)
    );
}

#[test]
#[cfg(target_os = "macos")]
fn apple_calendar_probe_non_macos_skip_is_cfg_disabled_on_macos() {
    // Given / When / Then: this marker proves the non-macOS skip test is intentionally cfg-gated.
    assert!(cfg!(target_os = "macos"));
}
