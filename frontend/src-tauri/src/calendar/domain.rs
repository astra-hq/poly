use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalendarProviderKind {
    Apple,
    Fake,
    Google,
    Microsoft,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CalendarEventSource {
    pub provider_kind: CalendarProviderKind,
    pub calendar_id: String,
}

impl CalendarEventSource {
    pub fn new(provider_kind: CalendarProviderKind, calendar_id: impl Into<String>) -> Self {
        Self {
            provider_kind,
            calendar_id: calendar_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CalendarEventId(String);

impl CalendarEventId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventOccurrenceKey(String);

impl EventOccurrenceKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarInstant(DateTime<Utc>);

impl CalendarInstant {
    pub const fn from_utc(value: DateTime<Utc>) -> Self {
        Self(value)
    }

    pub const fn as_utc(self) -> DateTime<Utc> {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarTimeRange {
    pub start: CalendarInstant,
    pub end: CalendarInstant,
}

impl CalendarTimeRange {
    pub fn new(
        start: CalendarInstant,
        end: CalendarInstant,
    ) -> Result<Self, CalendarTimeRangeError> {
        if start >= end {
            return Err(CalendarTimeRangeError::StartAtOrAfterEnd { start, end });
        }

        Ok(Self { start, end })
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CalendarTimeRangeError {
    #[error("calendar start {start:?} must be before end {end:?}")]
    StartAtOrAfterEnd {
        start: CalendarInstant,
        end: CalendarInstant,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventResponseStatus {
    Accepted,
    Tentative,
    Declined,
    NeedsAction,
    Delegated,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventCategory {
    Timed,
    AllDay,
    OutOfOffice,
    FocusTime,
    Reminder,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarMeetingLink(String);

impl CalendarMeetingLink {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventBody(String);

impl CalendarEventBody {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarOrganizer {
    pub email: String,
    pub display_name: Option<String>,
}

impl CalendarOrganizer {
    pub fn new(email: impl Into<String>, display_name: Option<String>) -> Self {
        Self {
            email: email.into(),
            display_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarAttendee {
    pub email: String,
    pub display_name: Option<String>,
    pub response_status: EventResponseStatus,
}

impl CalendarAttendee {
    pub fn new(
        email: impl Into<String>,
        display_name: Option<String>,
        response_status: EventResponseStatus,
    ) -> Self {
        Self {
            email: email.into(),
            display_name,
            response_status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventDetails {
    pub title: String,
    pub body: Option<CalendarEventBody>,
    pub location: Option<String>,
    pub meeting_link: Option<CalendarMeetingLink>,
    pub organizer: Option<CalendarOrganizer>,
    pub attendees: Vec<CalendarAttendee>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEvent {
    pub source: CalendarEventSource,
    pub id: CalendarEventId,
    pub occurrence_key: EventOccurrenceKey,
    pub details: CalendarEventDetails,
    pub time_range: CalendarTimeRange,
    pub response_status: EventResponseStatus,
    pub category: EventCategory,
    pub is_cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderEventCursor {
    Start,
    After(String),
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventPage {
    pub events: Vec<CalendarEvent>,
    pub next_cursor: ProviderEventCursor,
}

pub trait CalendarClock: Send + Sync {
    fn now(&self) -> CalendarInstant;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl CalendarClock for SystemClock {
    fn now(&self) -> CalendarInstant {
        CalendarInstant::from_utc(chrono::Utc::now())
    }
}
