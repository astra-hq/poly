use async_trait::async_trait;
use chrono::DateTime;

use crate::calendar::domain::{
    CalendarAttendee, CalendarEvent, CalendarEventBody, CalendarEventDetails, CalendarEventId,
    CalendarEventPage, CalendarEventSource, CalendarInstant, CalendarMeetingLink,
    CalendarOrganizer, CalendarProviderKind, CalendarTimeRange, EventCategory, EventOccurrenceKey,
    EventResponseStatus, ProviderEventCursor,
};
use crate::calendar::provider::{CalendarProvider, CalendarProviderError, CalendarProviderResult};

#[cfg(target_os = "macos")]
use crate::calendar::macos_eventkit;
#[cfg(target_os = "macos")]
use crate::calendar::types::{CalendarProbeErrorKind, RawCalendarEvent};

#[derive(Debug, Clone, Copy, Default)]
pub struct AppleCalendarProvider;

#[async_trait]
impl CalendarProvider for AppleCalendarProvider {
    fn provider_kind(&self) -> CalendarProviderKind {
        CalendarProviderKind::Apple
    }

    #[cfg(target_os = "macos")]
    async fn list_upcoming_events(
        &self,
        window: CalendarTimeRange,
        cursor: ProviderEventCursor,
    ) -> CalendarProviderResult<CalendarEventPage> {
        match cursor {
            ProviderEventCursor::Start => {
                let start_epoch = window.start.as_utc().timestamp() as f64;
                let end_epoch = window.end.as_utc().timestamp() as f64;

                let raw_events = macos_eventkit::fetch_events(start_epoch, end_epoch)
                    .map_err(map_eventkit_error)?;

                let events: Vec<CalendarEvent> = raw_events
                    .into_iter()
                    .filter_map(convert_raw_event)
                    .collect();

                Ok(CalendarEventPage {
                    events,
                    next_cursor: ProviderEventCursor::End,
                })
            }
            ProviderEventCursor::After(_) | ProviderEventCursor::End => Ok(CalendarEventPage {
                events: Vec::new(),
                next_cursor: ProviderEventCursor::End,
            }),
        }
    }

    #[cfg(not(target_os = "macos"))]
    async fn list_upcoming_events(
        &self,
        _window: CalendarTimeRange,
        _cursor: ProviderEventCursor,
    ) -> CalendarProviderResult<CalendarEventPage> {
        Err(CalendarProviderError::UnsupportedPlatform {
            provider_kind: CalendarProviderKind::Apple,
        })
    }

    #[cfg(target_os = "macos")]
    async fn event_details(
        &self,
        id: &CalendarEventId,
        occurrence_key: &EventOccurrenceKey,
    ) -> CalendarProviderResult<CalendarEvent> {
        let raw = macos_eventkit::fetch_event_by_identifier(id.as_str())
            .map_err(map_eventkit_error)?
            .ok_or_else(|| CalendarProviderError::EventNotFound {
                id: id.clone(),
                occurrence_key: occurrence_key.clone(),
            })?;

        convert_raw_event(raw).ok_or_else(|| CalendarProviderError::EventNotFound {
            id: id.clone(),
            occurrence_key: occurrence_key.clone(),
        })
    }

    #[cfg(not(target_os = "macos"))]
    async fn event_details(
        &self,
        _id: &CalendarEventId,
        _occurrence_key: &EventOccurrenceKey,
    ) -> CalendarProviderResult<CalendarEvent> {
        Err(CalendarProviderError::UnsupportedPlatform {
            provider_kind: CalendarProviderKind::Apple,
        })
    }
}

#[cfg(target_os = "macos")]
fn map_eventkit_error(error: crate::calendar::types::CalendarProbeError) -> CalendarProviderError {
    match error.kind {
        CalendarProbeErrorKind::PermissionDenied | CalendarProbeErrorKind::Restricted => {
            CalendarProviderError::PermissionDenied {
                provider_kind: CalendarProviderKind::Apple,
            }
        }
        CalendarProbeErrorKind::UnsupportedPlatform => CalendarProviderError::UnsupportedPlatform {
            provider_kind: CalendarProviderKind::Apple,
        },
        _ => CalendarProviderError::RequestFailed {
            message: error.message,
        },
    }
}

#[cfg(target_os = "macos")]
fn convert_raw_event(raw: RawCalendarEvent) -> Option<CalendarEvent> {
    let start_ts = raw.start?;
    let end_ts = raw.end?;

    let start = CalendarInstant::from_utc(DateTime::from_timestamp(start_ts as i64, 0)?);
    let end = CalendarInstant::from_utc(DateTime::from_timestamp(end_ts as i64, 0)?);

    let time_range = CalendarTimeRange::new(start, end).ok()?;
    let id = CalendarEventId::new(&raw.identifier);
    let occurrence_key = EventOccurrenceKey::new(format!("{}|{}", raw.identifier, start_ts as i64));

    let response_status = if raw.is_cancelled {
        EventResponseStatus::Declined
    } else {
        EventResponseStatus::Accepted
    };

    let category = if raw.is_all_day {
        EventCategory::AllDay
    } else {
        EventCategory::Timed
    };

    let source = CalendarEventSource::new(
        CalendarProviderKind::Apple,
        raw.calendar_id.unwrap_or_default(),
    );

    let meeting_link = raw.url.map(CalendarMeetingLink::new);
    let body = raw.notes.map(CalendarEventBody::new);

    let organizer = raw
        .organizer_name
        .map(|name| CalendarOrganizer::new(raw.organizer_email.unwrap_or_default(), Some(name)));

    let attendees: Vec<CalendarAttendee> = raw
        .attendees
        .into_iter()
        .map(|email_or_name| {
            CalendarAttendee::new(email_or_name, None, EventResponseStatus::Unknown)
        })
        .collect();

    Some(CalendarEvent {
        source,
        id,
        occurrence_key,
        details: CalendarEventDetails {
            title: raw.title.unwrap_or_else(|| "Untitled".to_string()),
            body,
            location: raw.location,
            meeting_link,
            organizer,
            attendees,
        },
        time_range,
        response_status,
        category,
        is_cancelled: raw.is_cancelled,
    })
}
