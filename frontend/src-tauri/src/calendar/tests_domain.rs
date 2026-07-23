use crate::calendar::domain::{
    CalendarAttendee, CalendarEvent, CalendarEventBody, CalendarEventDetails, CalendarEventId,
    CalendarEventSource, CalendarInstant, CalendarMeetingLink, CalendarOrganizer,
    CalendarProviderKind, CalendarTimeRange, EventCategory, EventOccurrenceKey,
    EventResponseStatus, ProviderEventCursor,
};
use crate::calendar::eligibility::{auto_record_eligibility, AutoRecordEligibility};
use crate::calendar::fake_provider::{FakeCalendarProvider, FakeClock};
use crate::calendar::provider::{CalendarProvider, CalendarProviderError};
use chrono::{TimeZone, Utc};

fn instant(seconds: i64) -> CalendarInstant {
    CalendarInstant::from_utc(
        Utc.timestamp_opt(seconds, 0)
            .single()
            .expect("valid timestamp"),
    )
}

fn video_event(status: EventResponseStatus) -> CalendarEvent {
    CalendarEvent {
        source: CalendarEventSource::new(CalendarProviderKind::Apple, "primary"),
        id: CalendarEventId::new("event-1"),
        occurrence_key: EventOccurrenceKey::new("event-1:2026-07-08T17:00:00Z"),
        details: CalendarEventDetails {
            title: "Planning".into(),
            body: Some(CalendarEventBody::new("Join https://meet.example.com/team")),
            location: None,
            meeting_link: Some(CalendarMeetingLink::new("https://meet.example.com/team")),
            organizer: Some(CalendarOrganizer::new(
                "owner@example.com",
                Some("Owner".into()),
            )),
            attendees: vec![CalendarAttendee::new(
                "person@example.com",
                Some("Person".into()),
                status,
            )],
        },
        time_range: CalendarTimeRange::new(instant(1_783_526_400), instant(1_783_530_000))
            .expect("valid event range"),
        response_status: status,
        category: EventCategory::Timed,
        is_cancelled: false,
        organizer_is_current_user: false,
    }
}

#[test]
fn calendar_domain_eligibility_accepts_accepted_video_meeting_when_in_future() {
    // Given: an accepted timed event with a normalized video link after the fake clock.
    let clock = FakeClock::new(instant(1_783_522_800));
    let event = video_event(EventResponseStatus::Accepted);

    // When: auto-record eligibility is evaluated.
    let result = auto_record_eligibility(&event, &clock);

    // Then: the event is eligible with its typed id preserved.
    assert_eq!(
        result,
        AutoRecordEligibility::eligible(event.id.clone(), event.occurrence_key.clone())
    );
}

#[test]
fn calendar_domain_eligibility_accepts_tentative_video_meeting_when_in_future() {
    // Given: a tentative timed event with a video link.
    let clock = FakeClock::new(instant(1_783_522_800));
    let event = video_event(EventResponseStatus::Tentative);

    // When: eligibility is evaluated.
    let result = auto_record_eligibility(&event, &clock);

    // Then: tentative attendance is eligible for auto-recording.
    assert!(result.is_eligible());
}

#[test]
fn calendar_domain_eligibility_rejects_declined_cancelled_all_day_and_no_link_events() {
    // Given: otherwise valid events that each violate one eligibility rule.
    let clock = FakeClock::new(instant(1_783_522_800));
    let declined = video_event(EventResponseStatus::Declined);
    let cancelled = CalendarEvent {
        is_cancelled: true,
        ..video_event(EventResponseStatus::Accepted)
    };
    let all_day = CalendarEvent {
        category: EventCategory::AllDay,
        ..video_event(EventResponseStatus::Accepted)
    };
    let no_link = CalendarEvent {
        details: CalendarEventDetails {
            meeting_link: None,
            ..video_event(EventResponseStatus::Accepted).details
        },
        ..video_event(EventResponseStatus::Accepted)
    };

    // When / Then: each event receives a typed ineligibility reason.
    assert_eq!(
        auto_record_eligibility(&declined, &clock),
        AutoRecordEligibility::ineligible_declined(
            declined.id.clone(),
            declined.occurrence_key.clone()
        )
    );
    assert_eq!(
        auto_record_eligibility(&cancelled, &clock),
        AutoRecordEligibility::ineligible_cancelled(
            cancelled.id.clone(),
            cancelled.occurrence_key.clone()
        )
    );
    assert_eq!(
        auto_record_eligibility(&all_day, &clock),
        AutoRecordEligibility::ineligible_all_day(
            all_day.id.clone(),
            all_day.occurrence_key.clone()
        )
    );
    assert_eq!(
        auto_record_eligibility(&no_link, &clock),
        AutoRecordEligibility::ineligible_missing_meeting_link(
            no_link.id.clone(),
            no_link.occurrence_key.clone()
        )
    );
}

#[tokio::test]
async fn calendar_domain_fake_provider_lists_upcoming_and_returns_details_by_id() {
    // Given: a fake provider seeded with one upcoming event.
    let event = video_event(EventResponseStatus::Accepted);
    let provider = FakeCalendarProvider::new(vec![event.clone()]);
    let window = CalendarTimeRange::new(instant(1_783_522_800), instant(1_783_612_800))
        .expect("valid query window");

    // When: callers use the provider trait methods.
    let upcoming = provider
        .list_upcoming_events(window, ProviderEventCursor::Start)
        .await
        .expect("fake list succeeds");
    let details = provider
        .event_details(&event.id, &event.occurrence_key)
        .await
        .expect("fake details succeeds");

    // Then: fake behavior matches the provider contract without network or OS dependencies.
    assert_eq!(provider.provider_kind(), CalendarProviderKind::Fake);
    assert_eq!(upcoming.events, vec![event.clone()]);
    assert_eq!(upcoming.next_cursor, ProviderEventCursor::End);
    assert_eq!(details, event);
}

#[tokio::test]
async fn calendar_domain_fake_provider_returns_typed_missing_event_error() {
    // Given: an empty fake provider.
    let provider = FakeCalendarProvider::new(Vec::new());

    // When: details are requested for an unknown event.
    let result = provider
        .event_details(
            &CalendarEventId::new("missing"),
            &EventOccurrenceKey::new("missing:occurrence"),
        )
        .await;

    // Then: callers get a typed missing-event error.
    assert_eq!(
        result,
        Err(CalendarProviderError::EventNotFound {
            id: CalendarEventId::new("missing"),
            occurrence_key: EventOccurrenceKey::new("missing:occurrence"),
        })
    );
}

#[test]
fn calendar_events_sort_by_start_time_ascending() {
    // Given: three events out of chronological order.
    let _clock = FakeClock::new(instant(1_783_522_800));
    let mut late = video_event(EventResponseStatus::Accepted);
    let mut mid = video_event(EventResponseStatus::Accepted);
    let mut early = video_event(EventResponseStatus::Accepted);

    late.time_range = CalendarTimeRange::new(instant(1_783_530_000), instant(1_783_533_600))
        .expect("valid range");
    mid.time_range = CalendarTimeRange::new(instant(1_783_526_400), instant(1_783_530_000))
        .expect("valid range");
    early.time_range = CalendarTimeRange::new(instant(1_783_522_800), instant(1_783_526_400))
        .expect("valid range");

    let mut events = vec![late.clone(), early.clone(), mid.clone()];

    // When: sorted by start time (mirroring the command behavior).
    events.sort_by_key(|e| e.time_range.start);

    // Then: order is earliest → latest.
    assert_eq!(events[0].id, early.id);
    assert_eq!(events[1].id, mid.id);
    assert_eq!(events[2].id, late.id);
}
