//! Privacy-safe calendar analytics / logging guardrails.
//!
//! The sanitizer ensures that calendar title, attendee emails, invite body,
//! meeting URL, organizer emails, and raw provider payloads are never sent to
//! analytics or written to log output.  Only coarse booleans, counts, and
//! statuses may leave this module.
//!
//! ## Allowed fields
//!
//! | Field | Type | Example |
//! |---|---|---|
//! | `calendar_enabled` | bool | `true` |
//! | `auto_record_enabled` | bool | `false` |
//! | `provider_kind` | enum variant | `apple` |
//! | `skip_reason_category` | enum variant | `all_day` |
//! | `permission_status` | enum variant | `full_access` |
//! | `event_count` | usize | `42` |
//! | `candidate_count` | usize | `5` |
//! | `eligible_count` | usize | `2` |
//! | `platform_supported` | bool | `true` |
//! | `lookahead_window_minutes` | i64 | `60` |

use std::collections::HashMap;

use crate::calendar::domain::{CalendarEvent, CalendarProviderKind};
use crate::calendar::eligibility::{AutoRecordEligibility, AutoRecordIneligibilityReason};
use crate::calendar::types::CalendarPermissionStatus;

/// The set of calendar fields that are **forbidden** in analytics and logs.
///
/// This list is consulted by `CalendarPrivacySanitizer::redacts()` and the
/// analytics `sanitize_analytics_properties` function.  Any key that
/// matches (case-sensitive) is stripped before data leaves the process.
pub const FORBIDDEN_CALENDAR_KEYS: &[&str] = &[
    // ----- event identification (sensitive) -----
    "event_title",
    "eventTitle",
    "calendar_title",
    "calendarTitle",
    "meeting_title",
    "meetingTitle",
    "meeting_name",
    "meetingName",
    // ----- attendees & organizer -----
    "attendee",
    "attendee_email",
    "attendeeEmail",
    "attendee_name",
    "attendeeName",
    "attendees",
    "organizer",
    "organizer_email",
    "organizerEmail",
    "organizer_name",
    "organizerName",
    // ----- meeting link / url -----
    "meeting_url",
    "meetingUrl",
    "meeting_link",
    "meetingLink",
    "join_url",
    "joinUrl",
    "conference_url",
    "conferenceUrl",
    // ----- body / description -----
    "invite_body",
    "inviteBody",
    "event_body",
    "eventBody",
    "event_notes",
    "eventNotes",
    "calendar_notes",
    "calendarNotes",
    "description",
    // ----- raw provider payloads -----
    "raw_event",
    "rawEvent",
    "provider_payload",
    "providerPayload",
    "raw_payload",
    "rawPayload",
    "event_data",
    "eventData",
    "source_data",
    "sourceData",
    // ----- location (may be sensitive) -----
    "location",
    "event_location",
    "eventLocation",
];

// ── Safe property builders ────────────────────────────────────────────────

/// Build a privacy-safe analytics property map from a `CalendarConfig`.
///
/// Only coarse configuration booleans and the provider kind are emitted.
pub fn calendar_config_properties(
    calendar_enabled: bool,
    auto_record_enabled: bool,
    provider_kind: CalendarProviderKind,
    lookahead_window_minutes: i64,
) -> HashMap<String, String> {
    let mut props = HashMap::new();
    props.insert("calendar_enabled".to_string(), calendar_enabled.to_string());
    props.insert(
        "auto_record_enabled".to_string(),
        auto_record_enabled.to_string(),
    );
    props.insert(
        "provider_kind".to_string(),
        provider_kind_to_str(provider_kind).to_string(),
    );
    props.insert(
        "lookahead_window_minutes".to_string(),
        lookahead_window_minutes.to_string(),
    );
    props
}

/// Build a privacy-safe property map from a permission status check.
pub fn calendar_permission_properties(status: CalendarPermissionStatus) -> HashMap<String, String> {
    let mut props = HashMap::new();
    props.insert(
        "permission_granted".to_string(),
        status.can_read_events().to_string(),
    );
    props.insert(
        "permission_status".to_string(),
        permission_status_to_str(status).to_string(),
    );
    props
}

/// Build a privacy-safe property map from an eligibility result.
///
/// Emits whether the event is eligible and, if ineligible, a coarse
/// reason category (never the event title, link, or attendee list).
pub fn eligibility_properties(eligibility: &AutoRecordEligibility) -> HashMap<String, String> {
    let mut props = HashMap::new();
    props.insert(
        "eligible".to_string(),
        eligibility.is_eligible().to_string(),
    );
    if let AutoRecordEligibility::Ineligible { reason, .. } = eligibility {
        props.insert(
            "skip_reason_category".to_string(),
            ineligibility_reason_to_str(*reason).to_string(),
        );
    }
    props
}

/// Build a privacy-safe property map summarising a batch of candidates.
///
/// Only aggregate counts are emitted — no individual event data.
pub fn candidate_batch_properties(
    total_count: usize,
    eligible_count: usize,
) -> HashMap<String, String> {
    let mut props = HashMap::new();
    props.insert("candidate_count".to_string(), total_count.to_string());
    props.insert("eligible_count".to_string(), eligible_count.to_string());
    props
}

/// Build a privacy-safe property map from a provider health check.
pub fn provider_health_properties(
    platform_supported: bool,
    permission_granted: bool,
    event_count: Option<usize>,
) -> HashMap<String, String> {
    let mut props = HashMap::new();
    props.insert(
        "platform_supported".to_string(),
        platform_supported.to_string(),
    );
    props.insert(
        "permission_granted".to_string(),
        permission_granted.to_string(),
    );
    if let Some(count) = event_count {
        props.insert("event_count".to_string(), count.to_string());
    }
    props
}

// ── Sanitizer ─────────────────────────────────────────────────────────────

/// A privacy guard that ensures calendar-sensitive data is never emitted.
///
/// `CalendarPrivacySanitizer` wraps a `CalendarEvent` and only exposes
/// coarse, non-sensitive fields through its public API.  Attempts to read
/// `title`, `body`, `meeting_link`, `organizer`, or `attendees` are
/// compile-time blocked — these fields are private.
#[derive(Debug, Clone)]
pub struct CalendarPrivacySanitizer {
    provider_kind: CalendarProviderKind,
    is_cancelled: bool,
    category: String,
    response_status: String,
    has_meeting_link: bool,
    attendee_count: usize,
}

impl CalendarPrivacySanitizer {
    /// Create a sanitizer from a calendar event, stripping all sensitive data.
    pub fn from_event(event: &CalendarEvent) -> Self {
        Self {
            provider_kind: event.source.provider_kind,
            is_cancelled: event.is_cancelled,
            category: format!("{:?}", event.category),
            response_status: format!("{:?}", event.response_status),
            has_meeting_link: event.details.meeting_link.is_some(),
            attendee_count: event.details.attendees.len(),
        }
    }

    /// Convert to a `HashMap` suitable for analytics use.
    ///
    /// Only coarse fields are emitted.  No title, link, body, names, or
    /// emails appear in the output.
    pub fn to_properties(&self) -> HashMap<String, String> {
        let mut props = HashMap::new();
        props.insert(
            "provider_kind".to_string(),
            provider_kind_to_str(self.provider_kind).to_string(),
        );
        props.insert("is_cancelled".to_string(), self.is_cancelled.to_string());
        props.insert("category".to_string(), self.category.clone());
        props.insert("response_status".to_string(), self.response_status.clone());
        props.insert(
            "has_meeting_link".to_string(),
            self.has_meeting_link.to_string(),
        );
        props.insert(
            "attendee_count".to_string(),
            self.attendee_count.to_string(),
        );
        props
    }

    /// Check whether a given analytics property key is forbidden.
    ///
    /// Returns `true` if the key represents sensitive calendar data that
    /// must never appear in analytics or logs.
    pub fn redacts(key: &str) -> bool {
        FORBIDDEN_CALENDAR_KEYS
            .iter()
            .any(|&forbidden| forbidden == key)
    }

    // ── coarse accessors (none of these leak PII) ──

    pub fn provider_kind(&self) -> CalendarProviderKind {
        self.provider_kind
    }

    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled
    }

    pub fn has_meeting_link(&self) -> bool {
        self.has_meeting_link
    }

    pub fn attendee_count(&self) -> usize {
        self.attendee_count
    }
}

// ── string helpers ────────────────────────────────────────────────────────

fn provider_kind_to_str(kind: CalendarProviderKind) -> &'static str {
    match kind {
        CalendarProviderKind::Apple => "apple",
        CalendarProviderKind::Fake => "fake",
        CalendarProviderKind::Google => "google",
        CalendarProviderKind::Microsoft => "microsoft",
    }
}

fn permission_status_to_str(status: CalendarPermissionStatus) -> &'static str {
    match status {
        CalendarPermissionStatus::NotDetermined => "not_determined",
        CalendarPermissionStatus::Restricted => "restricted",
        CalendarPermissionStatus::Denied => "denied",
        CalendarPermissionStatus::Authorized => "authorized",
        CalendarPermissionStatus::FullAccess => "full_access",
        CalendarPermissionStatus::WriteOnly => "write_only",
        CalendarPermissionStatus::UnsupportedPlatform => "unsupported_platform",
        CalendarPermissionStatus::Unknown => "unknown",
    }
}

fn ineligibility_reason_to_str(reason: AutoRecordIneligibilityReason) -> &'static str {
    match reason {
        AutoRecordIneligibilityReason::Declined => "declined",
        AutoRecordIneligibilityReason::Cancelled => "cancelled",
        AutoRecordIneligibilityReason::AllDay => "all_day",
        AutoRecordIneligibilityReason::MissingMeetingLink => "missing_meeting_link",
        AutoRecordIneligibilityReason::AlreadyEnded => "already_ended",
        AutoRecordIneligibilityReason::UnsupportedResponseStatus => "unsupported_response_status",
    }
}

// ── tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::domain::{
        CalendarAttendee, CalendarEventBody, CalendarEventDetails, CalendarEventId,
        CalendarEventSource, CalendarInstant, CalendarMeetingLink, CalendarOrganizer,
        CalendarTimeRange, EventCategory, EventOccurrenceKey, EventResponseStatus,
    };

    fn sample_event() -> CalendarEvent {
        let now = chrono::Utc::now();
        let start = CalendarInstant::from_utc(now);
        let end = CalendarInstant::from_utc(now + chrono::Duration::hours(1));
        CalendarEvent {
            source: CalendarEventSource::new(CalendarProviderKind::Apple, "cal-1"),
            id: CalendarEventId::new("evt-1"),
            occurrence_key: EventOccurrenceKey::new("evt-1|1234567890"),
            details: CalendarEventDetails {
                title: "Board Strategy Session".to_string(),
                body: Some(CalendarEventBody::new(
                    "Confidential: discuss Q3 acquisition targets and layoff plans",
                )),
                location: Some("Room 301, Acme Tower".to_string()),
                meeting_link: Some(CalendarMeetingLink::new(
                    "https://zoom.us/j/123456789?pwd=secret",
                )),
                organizer: Some(CalendarOrganizer::new(
                    "ceo@acme-corp.com",
                    Some("Jane CEO".to_string()),
                )),
                attendees: vec![
                    CalendarAttendee::new(
                        "cfo@acme-corp.com",
                        Some("Bob CFO".to_string()),
                        EventResponseStatus::Accepted,
                    ),
                    CalendarAttendee::new(
                        "cto@acme-corp.com",
                        Some("Alice CTO".to_string()),
                        EventResponseStatus::Tentative,
                    ),
                ],
            },
            time_range: CalendarTimeRange::new(start, end).unwrap(),
            response_status: EventResponseStatus::Accepted,
            category: EventCategory::Timed,
            is_cancelled: false,
        }
    }

    #[test]
    fn sanitizer_strips_all_sensitive_fields_from_event() {
        let event = sample_event();
        let sanitized = CalendarPrivacySanitizer::from_event(&event);

        // Coarse fields are preserved.
        assert_eq!(sanitized.provider_kind(), CalendarProviderKind::Apple);
        assert!(!sanitized.is_cancelled());
        assert!(sanitized.has_meeting_link());
        assert_eq!(sanitized.attendee_count(), 2);

        // Properties map must NOT contain any sensitive data.
        let props = sanitized.to_properties();

        // These are the allowed keys.
        assert_eq!(
            props.get("provider_kind").map(|s| s.as_str()),
            Some("apple")
        );
        assert_eq!(props.get("is_cancelled").map(|s| s.as_str()), Some("false"));
        assert_eq!(
            props.get("has_meeting_link").map(|s| s.as_str()),
            Some("true")
        );
        assert_eq!(props.get("attendee_count").map(|s| s.as_str()), Some("2"));

        // Sensitive keys must never appear.
        let sensitive_keys = [
            "event_title",
            "meeting_url",
            "attendee_email",
            "organizer_email",
            "invite_body",
            "title",
        ];
        for key in &sensitive_keys {
            assert!(
                !props.contains_key(*key),
                "sensitive key '{}' leaked into analytics properties: {:?}",
                key,
                props.get(*key),
            );
        }
    }

    #[test]
    fn redacts_returns_true_for_forbidden_keys() {
        assert!(CalendarPrivacySanitizer::redacts("event_title"));
        assert!(CalendarPrivacySanitizer::redacts("meeting_url"));
        assert!(CalendarPrivacySanitizer::redacts("attendee_email"));
        assert!(CalendarPrivacySanitizer::redacts("organizer_email"));
        assert!(CalendarPrivacySanitizer::redacts("invite_body"));
        assert!(CalendarPrivacySanitizer::redacts("provider_payload"));
        assert!(CalendarPrivacySanitizer::redacts("meetingLink"));
        assert!(CalendarPrivacySanitizer::redacts("rawEvent"));
    }

    #[test]
    fn redacts_returns_false_for_safe_keys() {
        assert!(!CalendarPrivacySanitizer::redacts("calendar_enabled"));
        assert!(!CalendarPrivacySanitizer::redacts("auto_record_enabled"));
        assert!(!CalendarPrivacySanitizer::redacts("provider_kind"));
        assert!(!CalendarPrivacySanitizer::redacts("skip_reason_category"));
        assert!(!CalendarPrivacySanitizer::redacts("candidate_count"));
        assert!(!CalendarPrivacySanitizer::redacts("eligible_count"));
        assert!(!CalendarPrivacySanitizer::redacts("event_count"));
    }

    #[test]
    fn sanitizer_does_not_expose_title_body_link_organizer_or_attendees() {
        let event = sample_event();
        let sanitized = CalendarPrivacySanitizer::from_event(&event);

        // The sanitizer has NO public methods that return title, body,
        // meeting link URL, organizer email, or attendee emails.
        let props = sanitized.to_properties();

        // Assert no value in the properties map contains the sensitive data.
        for (key, value) in &props {
            assert!(
                !value.contains("Board Strategy"),
                "property '{}' contains event title: '{}'",
                key,
                value
            );
            assert!(
                !value.contains("acme-corp.com"),
                "property '{}' contains an email address: '{}'",
                key,
                value
            );
            assert!(
                !value.contains("zoom.us"),
                "property '{}' contains a meeting link: '{}'",
                key,
                value
            );
            assert!(
                !value.contains("acquisition"),
                "property '{}' contains invite body content: '{}'",
                key,
                value
            );
        }
    }

    #[test]
    fn config_properties_only_include_coarse_fields() {
        let props = calendar_config_properties(true, false, CalendarProviderKind::Apple, 60);
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
        assert_eq!(props.len(), 4);
    }

    #[test]
    fn eligibility_properties_emit_only_boolean_and_reason_category() {
        let id = CalendarEventId::new("evt-1");
        let key = EventOccurrenceKey::new("evt-1|123");
        let eligible = AutoRecordEligibility::eligible(id.clone(), key.clone());
        let ineligible = AutoRecordEligibility::ineligible_all_day(id.clone(), key.clone());

        let eligible_props = eligibility_properties(&eligible);
        assert_eq!(
            eligible_props.get("eligible").map(|s| s.as_str()),
            Some("true")
        );
        assert!(!eligible_props.contains_key("skip_reason_category"));
        assert!(!eligible_props.contains_key("event_title"));
        assert!(!eligible_props.contains_key("event_id"));

        let ineligible_props = eligibility_properties(&ineligible);
        assert_eq!(
            ineligible_props.get("eligible").map(|s| s.as_str()),
            Some("false")
        );
        assert_eq!(
            ineligible_props
                .get("skip_reason_category")
                .map(|s| s.as_str()),
            Some("all_day")
        );
    }

    #[test]
    fn candidate_batch_properties_only_contain_counts() {
        let props = candidate_batch_properties(10, 3);
        assert_eq!(props.get("candidate_count").map(|s| s.as_str()), Some("10"));
        assert_eq!(props.get("eligible_count").map(|s| s.as_str()), Some("3"));
        assert_eq!(props.len(), 2);
    }

    #[test]
    fn permission_properties_never_contain_user_identifiers() {
        let props = calendar_permission_properties(CalendarPermissionStatus::FullAccess);
        assert_eq!(
            props.get("permission_granted").map(|s| s.as_str()),
            Some("true")
        );
        assert_eq!(
            props.get("permission_status").map(|s| s.as_str()),
            Some("full_access")
        );
        // No calendar IDs, user names, or device identifiers.
        for (key, _) in &props {
            assert!(!key.contains("calendar_id"));
            assert!(!key.contains("user"));
            assert!(!key.contains("device"));
        }
    }

    #[test]
    fn all_forbidden_keys_are_recognized_by_redacts() {
        for key in FORBIDDEN_CALENDAR_KEYS {
            assert!(
                CalendarPrivacySanitizer::redacts(key),
                "FORBIDDEN_CALENDAR_KEYS entry '{}' is not recognized by redacts()",
                key
            );
        }
    }
}
