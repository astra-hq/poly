use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarPermissionStatus {
    NotDetermined,
    Restricted,
    Denied,
    Authorized,
    FullAccess,
    WriteOnly,
    UnsupportedPlatform,
    Unknown,
}

impl CalendarPermissionStatus {
    pub const fn from_eventkit_status(raw: isize) -> Self {
        match raw {
            0 => Self::NotDetermined,
            1 => Self::Restricted,
            2 => Self::Denied,
            3 => Self::FullAccess,
            4 => Self::WriteOnly,
            _ => Self::Unknown,
        }
    }

    pub const fn can_read_events(self) -> bool {
        match self {
            Self::Authorized | Self::FullAccess => true,
            Self::NotDetermined
            | Self::Restricted
            | Self::Denied
            | Self::WriteOnly
            | Self::UnsupportedPlatform
            | Self::Unknown => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarProbeErrorKind {
    PermissionDenied,
    Restricted,
    UnsupportedPlatform,
    EventKitUnavailable,
    RequestFailed,
    UnknownAuthorizationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CalendarProbeError {
    pub kind: CalendarProbeErrorKind,
    pub message: String,
}

impl CalendarProbeError {
    pub(crate) fn new(kind: CalendarProbeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NormalizedCalendarEvent {
    pub identifier: String,
    pub title: Option<String>,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub attendees: Vec<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CalendarPermissionProbe {
    pub status: CalendarPermissionStatus,
    pub sample_event: Option<NormalizedCalendarEvent>,
    pub error: Option<CalendarProbeError>,
}

impl CalendarPermissionProbe {
    pub(crate) fn available(
        status: CalendarPermissionStatus,
        sample_event: Option<NormalizedCalendarEvent>,
    ) -> Self {
        Self {
            status,
            sample_event,
            error: None,
        }
    }

    pub(crate) fn unavailable(status: CalendarPermissionStatus, error: CalendarProbeError) -> Self {
        Self {
            status,
            sample_event: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawCalendarEvent {
    pub(crate) identifier: String,
    pub(crate) title: Option<String>,
    pub(crate) start: Option<f64>,
    pub(crate) end: Option<f64>,
    pub(crate) attendees: Vec<String>,
    pub(crate) notes: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) location: Option<String>,
    pub(crate) is_all_day: bool,
    pub(crate) is_cancelled: bool,
    pub(crate) calendar_id: Option<String>,
    pub(crate) organizer_name: Option<String>,
    pub(crate) organizer_email: Option<String>,
}

impl RawCalendarEvent {
    pub(crate) fn with_traditional_fields(
        identifier: String,
        title: Option<String>,
        start: Option<f64>,
        end: Option<f64>,
        attendees: Vec<String>,
        notes: Option<String>,
        url: Option<String>,
        location: Option<String>,
    ) -> Self {
        Self {
            identifier,
            title,
            start,
            end,
            attendees,
            notes,
            url,
            location,
            is_all_day: false,
            is_cancelled: false,
            calendar_id: None,
            organizer_name: None,
            organizer_email: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarProviderHealth {
    pub provider: String,
    pub platform_supported: bool,
    pub permission_granted: bool,
    pub permission_status: CalendarPermissionStatus,
    pub event_count: Option<usize>,
    pub error: Option<String>,
}

impl CalendarProviderHealth {
    pub fn ok(status: CalendarPermissionStatus, event_count: usize) -> Self {
        Self {
            provider: "apple".to_string(),
            platform_supported: true,
            permission_granted: status.can_read_events(),
            permission_status: status,
            event_count: Some(event_count),
            error: None,
        }
    }

    pub fn unsupported() -> Self {
        Self {
            provider: "apple".to_string(),
            platform_supported: false,
            permission_granted: false,
            permission_status: CalendarPermissionStatus::UnsupportedPlatform,
            event_count: None,
            error: Some("Apple Calendar EventKit is only available on macOS.".to_string()),
        }
    }

    pub fn error(status: CalendarPermissionStatus, message: impl Into<String>) -> Self {
        Self {
            provider: "apple".to_string(),
            platform_supported: cfg!(target_os = "macos"),
            permission_granted: false,
            permission_status: status,
            event_count: None,
            error: Some(message.into()),
        }
    }
}

impl From<RawCalendarEvent> for NormalizedCalendarEvent {
    fn from(event: RawCalendarEvent) -> Self {
        Self {
            identifier: event.identifier,
            title: event.title,
            start: event.start,
            end: event.end,
            attendees: event.attendees,
            notes: event.notes,
            url: event.url,
            location: event.location,
        }
    }
}
