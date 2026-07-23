use app_lib::analytics::analytics::sanitize_analytics_properties;
use app_lib::calendar::domain::{
    CalendarAttendee, CalendarEventBody, CalendarEventDetails, CalendarEventId,
    CalendarEventSource, CalendarInstant, CalendarMeetingLink, CalendarOrganizer,
    CalendarProviderKind, CalendarTimeRange, EventCategory, EventOccurrenceKey,
    EventResponseStatus,
};
use app_lib::calendar::eligibility::AutoRecordEligibility;
use app_lib::calendar::privacy::{
    calendar_config_properties, calendar_permission_properties, candidate_batch_properties,
    eligibility_properties, provider_health_properties, CalendarPrivacySanitizer,
    FORBIDDEN_CALENDAR_KEYS,
};
use app_lib::calendar::types::CalendarPermissionStatus;
use std::collections::HashMap;

// ── helpers ───────────────────────────────────────────────────────────────

fn sample_event() -> app_lib::calendar::domain::CalendarEvent {
    let now = chrono::Utc::now();
    let start = CalendarInstant::from_utc(now);
    let end = CalendarInstant::from_utc(now + chrono::Duration::hours(1));
    app_lib::calendar::domain::CalendarEvent {
        source: CalendarEventSource::new(CalendarProviderKind::Apple, "cal-work"),
        id: CalendarEventId::new("evt-sensitive-1"),
        occurrence_key: EventOccurrenceKey::new("evt-sensitive-1|1700000000"),
        details: CalendarEventDetails {
            title: "Acquisition Strategy — CONFIDENTIAL".to_string(),
            body: Some(CalendarEventBody::new(
                "Full board agenda: discuss acquisition of CompetitorX for $2B. Legal counsel present.",
            )),
            location: Some("C-Suite Boardroom".to_string()),
            meeting_link: Some(CalendarMeetingLink::new(
                "https://teams.microsoft.com/l/meetup-join/secret-token",
            )),
            organizer: Some(CalendarOrganizer::new(
                "chairman@fortune500.com",
                Some("Chairman".to_string()),
            )),
            attendees: vec![
                CalendarAttendee::new(
                    "ceo@fortune500.com",
                    Some("CEO".to_string()),
                    EventResponseStatus::Accepted,
                ),
                CalendarAttendee::new(
                    "cfo@fortune500.com",
                    Some("CFO".to_string()),
                    EventResponseStatus::Accepted,
                ),
                CalendarAttendee::new(
                    "legal@external-firm.com",
                    Some("External Counsel".to_string()),
                    EventResponseStatus::Tentative,
                ),
            ],
        },
        time_range: CalendarTimeRange::new(start, end).unwrap(),
        response_status: EventResponseStatus::Accepted,
        category: EventCategory::Timed,
        is_cancelled: false,
        organizer_is_current_user: false,
    }
}

// ── sanitizer tests ───────────────────────────────────────────────────────

#[test]
fn sanitizer_from_event_strips_all_pii() {
    let event = sample_event();
    let sanitized = CalendarPrivacySanitizer::from_event(&event);

    // Only coarse fields are accessible.
    assert_eq!(sanitized.provider_kind(), CalendarProviderKind::Apple);
    assert!(!sanitized.is_cancelled());
    assert!(sanitized.has_meeting_link());
    assert_eq!(sanitized.attendee_count(), 3);

    let props = sanitized.to_properties();

    // Verify every property value does NOT contain sensitive text.
    for (key, value) in &props {
        assert!(
            !value.contains("Acquisition"),
            "property '{}' leaked event title: '{}'",
            key,
            value
        );
        assert!(
            !value.contains("fortune500.com"),
            "property '{}' leaked attendee/organizer email: '{}'",
            key,
            value
        );
        assert!(
            !value.contains("teams.microsoft.com"),
            "property '{}' leaked meeting link: '{}'",
            key,
            value
        );
        assert!(
            !value.contains("CompetitorX"),
            "property '{}' leaked invite body: '{}'",
            key,
            value
        );
        assert!(
            !value.contains("Boardroom"),
            "property '{}' leaked location: '{}'",
            key,
            value
        );
    }

    // Verify only known-safe keys are present.
    let allowed_keys = [
        "provider_kind",
        "is_cancelled",
        "category",
        "response_status",
        "has_meeting_link",
        "attendee_count",
    ];
    for key in props.keys() {
        assert!(
            allowed_keys.contains(&key.as_str()),
            "unexpected key in sanitized properties: '{}'",
            key
        );
    }
}

#[test]
fn sanitizer_properties_are_all_strings() {
    let event = sample_event();
    let sanitized = CalendarPrivacySanitizer::from_event(&event);
    let props = sanitized.to_properties();

    // All values must be strings — no nested objects that could smuggle PII.
    for (_key, value) in &props {
        // Value is already a String by type; runtime check that it doesn't
        // contain JSON/structured data.
        assert!(
            !value.starts_with('{') && !value.starts_with('['),
            "property value looks like structured data: '{}'",
            value
        );
    }
}

// ── analytics sanitizer integration ───────────────────────────────────────

#[test]
fn analytics_sanitizer_drops_all_calendar_sensitive_keys() {
    let mut properties = HashMap::new();
    properties.insert("event_title".to_string(), "Board Strategy".to_string());
    properties.insert("eventTitle".to_string(), "Board Strategy".to_string());
    properties.insert(
        "meeting_url".to_string(),
        "https://zoom.us/j/123".to_string(),
    );
    properties.insert(
        "meetingLink".to_string(),
        "https://zoom.us/j/123".to_string(),
    );
    properties.insert("attendee_email".to_string(), "ceo@acme.com".to_string());
    properties.insert("organizer_email".to_string(), "chair@acme.com".to_string());
    properties.insert("organizerEmail".to_string(), "chair@acme.com".to_string());
    properties.insert(
        "invite_body".to_string(),
        "Confidential merger details".to_string(),
    );
    properties.insert(
        "inviteBody".to_string(),
        "Confidential merger details".to_string(),
    );
    properties.insert("raw_event".to_string(), r#"{"title":"secret"}"#.to_string());
    properties.insert(
        "provider_payload".to_string(),
        r#"{"title":"secret"}"#.to_string(),
    );
    properties.insert(
        "event_data".to_string(),
        r#"{"title":"secret"}"#.to_string(),
    );
    properties.insert("description".to_string(), "Project X kickoff".to_string());
    properties.insert("location".to_string(), "Room 301".to_string());
    properties.insert("calendarTitle".to_string(), "Acquisition Plan".to_string());
    properties.insert("attendees".to_string(), "ceo@a.com,cfo@a.com".to_string());
    properties.insert("organizer_name".to_string(), "Jane CEO".to_string());
    properties.insert(
        "conference_url".to_string(),
        "https://meet.google.com/abc".to_string(),
    );
    properties.insert("event_notes".to_string(), "Top secret notes".to_string());
    properties.insert("rawPayload".to_string(), "binary-blob".to_string());
    properties.insert("source_data".to_string(), "raw-ekevent-json".to_string());

    // Safe properties that should survive.
    properties.insert("calendar_enabled".to_string(), "true".to_string());
    properties.insert("auto_record_enabled".to_string(), "false".to_string());
    properties.insert("provider_kind".to_string(), "apple".to_string());
    properties.insert("eligible_count".to_string(), "5".to_string());
    properties.insert("meeting_id".to_string(), "meeting-123".to_string());
    properties.insert("app_version".to_string(), "1.0.0".to_string());

    let sanitized = sanitize_analytics_properties(properties);

    // All sensitive keys must be gone.
    for key in FORBIDDEN_CALENDAR_KEYS {
        assert!(
            !sanitized.contains_key(*key),
            "SENSITIVE_ANALYTICS_KEYS entry '{}' survived sanitization",
            key
        );
    }

    // Safe keys must remain.
    assert_eq!(
        sanitized.get("calendar_enabled").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(
        sanitized.get("auto_record_enabled").map(|s| s.as_str()),
        Some("false")
    );
    assert_eq!(
        sanitized.get("provider_kind").map(|s| s.as_str()),
        Some("apple")
    );
    assert_eq!(
        sanitized.get("eligible_count").map(|s| s.as_str()),
        Some("5")
    );
    assert_eq!(
        sanitized.get("meeting_id").map(|s| s.as_str()),
        Some("meeting-123")
    );
}

#[test]
fn analytics_sanitizer_retains_calendar_safe_keys() {
    let mut properties = HashMap::new();
    properties.insert("calendar_enabled".to_string(), "true".to_string());
    properties.insert("auto_record_enabled".to_string(), "true".to_string());
    properties.insert("provider_kind".to_string(), "apple".to_string());
    properties.insert("skip_reason_category".to_string(), "all_day".to_string());
    properties.insert("candidate_count".to_string(), "10".to_string());
    properties.insert("eligible_count".to_string(), "3".to_string());
    properties.insert("event_count".to_string(), "42".to_string());
    properties.insert("platform_supported".to_string(), "true".to_string());
    properties.insert("permission_granted".to_string(), "true".to_string());
    properties.insert("permission_status".to_string(), "full_access".to_string());
    properties.insert("is_cancelled".to_string(), "false".to_string());
    properties.insert("has_meeting_link".to_string(), "true".to_string());
    properties.insert("attendee_count".to_string(), "5".to_string());
    properties.insert("lookahead_window_minutes".to_string(), "60".to_string());

    let sanitized = sanitize_analytics_properties(properties);

    // Every safe key must survive.
    assert_eq!(
        sanitized.get("calendar_enabled").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(
        sanitized.get("auto_record_enabled").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(
        sanitized.get("provider_kind").map(|s| s.as_str()),
        Some("apple")
    );
    assert_eq!(
        sanitized.get("skip_reason_category").map(|s| s.as_str()),
        Some("all_day")
    );
    assert_eq!(
        sanitized.get("candidate_count").map(|s| s.as_str()),
        Some("10")
    );
    assert_eq!(
        sanitized.get("eligible_count").map(|s| s.as_str()),
        Some("3")
    );
    assert_eq!(sanitized.get("event_count").map(|s| s.as_str()), Some("42"));
    assert_eq!(
        sanitized.get("platform_supported").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(
        sanitized.get("permission_granted").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(
        sanitized.get("permission_status").map(|s| s.as_str()),
        Some("full_access")
    );
    assert_eq!(
        sanitized.get("is_cancelled").map(|s| s.as_str()),
        Some("false")
    );
    assert_eq!(
        sanitized.get("has_meeting_link").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(
        sanitized.get("attendee_count").map(|s| s.as_str()),
        Some("5")
    );
    assert_eq!(
        sanitized
            .get("lookahead_window_minutes")
            .map(|s| s.as_str()),
        Some("60")
    );
}

// ── property builder tests ────────────────────────────────────────────────

#[test]
fn calendar_config_properties_are_coarse_only() {
    let props = calendar_config_properties(true, false, CalendarProviderKind::Apple, 120);
    assert_eq!(props.len(), 4);
    assert_eq!(
        props.get("calendar_enabled").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(
        props.get("auto_record_enabled").map(|s| s.as_str()),
        Some("false")
    );
    assert_eq!(
        props.get("provider_kind").map(|s| s.as_str()),
        Some("apple")
    );
    assert_eq!(
        props.get("lookahead_window_minutes").map(|s| s.as_str()),
        Some("120")
    );
}

#[test]
fn permission_properties_are_platform_agnostic() {
    let props = calendar_permission_properties(CalendarPermissionStatus::UnsupportedPlatform);
    assert_eq!(
        props.get("permission_granted").map(|s| s.as_str()),
        Some("false")
    );
    assert_eq!(
        props.get("permission_status").map(|s| s.as_str()),
        Some("unsupported_platform")
    );
}

#[test]
fn eligibility_properties_emit_reason_for_each_ineligibility_category() {
    let id = CalendarEventId::new("evt-1");
    let key = EventOccurrenceKey::new("evt-1|123");

    let cases: Vec<(AutoRecordEligibility, &str)> = vec![
        (
            AutoRecordEligibility::ineligible_declined(id.clone(), key.clone()),
            "declined",
        ),
        (
            AutoRecordEligibility::ineligible_cancelled(id.clone(), key.clone()),
            "cancelled",
        ),
        (
            AutoRecordEligibility::ineligible_all_day(id.clone(), key.clone()),
            "all_day",
        ),
        (
            AutoRecordEligibility::ineligible_missing_meeting_link(id.clone(), key.clone()),
            "missing_meeting_link",
        ),
    ];

    for (eligibility, expected_reason) in cases {
        let props = eligibility_properties(&eligibility);
        assert_eq!(props.get("eligible").map(|s| s.as_str()), Some("false"));
        assert_eq!(
            props.get("skip_reason_category").map(|s| s.as_str()),
            Some(expected_reason),
            "wrong reason for {:?}",
            eligibility,
        );
        assert!(
            !props.contains_key("event_id"),
            "event_id leaked for {:?}",
            eligibility
        );
        assert!(
            !props.contains_key("event_title"),
            "event_title leaked for {:?}",
            eligibility
        );
    }
}

#[test]
fn candidate_batch_properties_only_emit_counts() {
    let props = candidate_batch_properties(42, 7);
    assert_eq!(props.len(), 2);
    assert_eq!(props.get("candidate_count").map(|s| s.as_str()), Some("42"));
    assert_eq!(props.get("eligible_count").map(|s| s.as_str()), Some("7"));
}

#[test]
fn provider_health_properties_never_contain_event_payloads() {
    let props = provider_health_properties(true, true, Some(150));
    assert_eq!(
        props.get("platform_supported").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(
        props.get("permission_granted").map(|s| s.as_str()),
        Some("true")
    );
    assert_eq!(props.get("event_count").map(|s| s.as_str()), Some("150"));

    // event_count is the only allowed "event" key.
    // No raw event payloads or calendar identifiers.
    for key in props.keys() {
        if key == "event_count" {
            continue;
        }
        assert!(
            !key.contains("event"),
            "unexpected event-related key '{}' in provider health properties",
            key,
        );
        assert!(
            !key.contains("calendar"),
            "unexpected calendar-related key '{}' in provider health properties",
            key,
        );
        assert!(!key.contains("raw"));
        assert!(!key.contains("payload"));
    }
}

// ── grep-equivalent checks ────────────────────────────────────────────────

/// Simulates a grep check: no FORBIDDEN_CALENDAR_KEYS value should
/// appear as a key in the safe analytics properties across any builder.
#[test]
fn grep_no_forbidden_keys_in_safe_property_builders() {
    // All safe property builders.
    let all_safe_maps: Vec<HashMap<String, String>> = vec![
        calendar_config_properties(true, false, CalendarProviderKind::Apple, 60),
        calendar_permission_properties(CalendarPermissionStatus::FullAccess),
        candidate_batch_properties(10, 3),
        provider_health_properties(true, true, Some(42)),
    ];

    for (idx, safe_map) in all_safe_maps.iter().enumerate() {
        for key in safe_map.keys() {
            for forbidden in FORBIDDEN_CALENDAR_KEYS {
                assert_ne!(
                    key.as_str(),
                    *forbidden,
                    "builder {} emitted forbidden key '{}'",
                    idx,
                    forbidden,
                );
            }
        }
    }
}

/// Verify that `sanitize_analytics_properties` actually strips the
/// extended keys — the existing test only checked the original 13.
#[test]
fn legacy_analytics_test_still_passes_with_extended_keys() {
    // Replicate the exact scenario from analytics::tests::analytics_properties_drop_sensitive_meeting_metadata
    // but also verify the new calendar keys are stripped.
    let mut properties = HashMap::new();
    properties.insert("meeting_title".to_string(), "Board Strategy".to_string());
    properties.insert("meetingTitle".to_string(), "Board Strategy".to_string());
    properties.insert("meeting_name".to_string(), "Client Call".to_string());
    properties.insert("meetingName".to_string(), "Client Call".to_string());
    properties.insert("file_name".to_string(), "acquisition.wav".to_string());
    properties.insert("filename".to_string(), "acquisition.wav".to_string());
    properties.insert("device_name".to_string(), "Jane's AirPods".to_string());
    properties.insert("user_agent".to_string(), "Mozilla/5.0".to_string());
    // New calendar keys that should also be stripped
    properties.insert("event_title".to_string(), "Sprint Planning".to_string());
    properties.insert(
        "meeting_url".to_string(),
        "https://zoom.us/j/123".to_string(),
    );
    properties.insert("attendee_email".to_string(), "dev@company.com".to_string());
    properties.insert("organizer_email".to_string(), "pm@company.com".to_string());
    properties.insert(
        "invite_body".to_string(),
        "Agenda: sprint review".to_string(),
    );
    properties.insert("raw_event".to_string(), r#"{"title":"secret"}"#.to_string());

    // Safe keys
    properties.insert("meeting_id".to_string(), "meeting-123".to_string());
    properties.insert("duration_seconds".to_string(), "125".to_string());
    properties.insert("segments_count".to_string(), "42".to_string());
    properties.insert("model_name".to_string(), "parakeet".to_string());
    properties.insert("platform".to_string(), "Windows".to_string());

    let sanitized = sanitize_analytics_properties(properties);

    // Legacy sensitive keys
    for key in [
        "meeting_title",
        "meetingTitle",
        "meeting_name",
        "meetingName",
        "file_name",
        "filename",
        "device_name",
        "user_agent",
    ] {
        assert!(
            !sanitized.contains_key(key),
            "legacy key '{}' survived",
            key
        );
    }

    // New calendar-sensitive keys
    for key in [
        "event_title",
        "meeting_url",
        "attendee_email",
        "organizer_email",
        "invite_body",
        "raw_event",
    ] {
        assert!(
            !sanitized.contains_key(key),
            "calendar key '{}' survived",
            key
        );
    }

    // Safe keys survived
    assert_eq!(
        sanitized.get("meeting_id"),
        Some(&"meeting-123".to_string())
    );
    assert_eq!(sanitized.get("duration_seconds"), Some(&"125".to_string()));
    assert_eq!(sanitized.get("segments_count"), Some(&"42".to_string()));
}

#[test]
fn forbidden_calendar_keys_are_consistent_with_analytics_sensitive_keys() {
    // Every key in FORBIDDEN_CALENDAR_KEYS must exist in the analytics
    // SENSITIVE_ANALYTICS_KEYS.  This test prevents the two lists from
    // drifting apart.
    use app_lib::analytics::analytics::SENSITIVE_ANALYTICS_KEYS;

    for forbidden in FORBIDDEN_CALENDAR_KEYS {
        assert!(
            SENSITIVE_ANALYTICS_KEYS.contains(forbidden),
            "FORBIDDEN_CALENDAR_KEYS key '{}' is missing from SENSITIVE_ANALYTICS_KEYS",
            forbidden,
        );
    }
}
