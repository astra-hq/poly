use async_trait::async_trait;
use thiserror::Error;

use crate::calendar::domain::{
    CalendarEvent, CalendarEventId, CalendarEventPage, CalendarProviderKind, CalendarTimeRange,
    EventOccurrenceKey, ProviderEventCursor,
};

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum CalendarProviderError {
    #[error("calendar permission denied for {provider_kind:?}")]
    PermissionDenied { provider_kind: CalendarProviderKind },
    #[error("calendar provider {provider_kind:?} is unsupported on this platform")]
    UnsupportedPlatform { provider_kind: CalendarProviderKind },
    #[error("calendar event {id:?} occurrence {occurrence_key:?} was not found")]
    EventNotFound {
        id: CalendarEventId,
        occurrence_key: EventOccurrenceKey,
    },
    #[error("calendar provider request failed: {message}")]
    RequestFailed { message: String },
}

pub type CalendarProviderResult<T> = Result<T, CalendarProviderError>;

#[async_trait]
pub trait CalendarProvider: Send + Sync {
    fn provider_kind(&self) -> CalendarProviderKind;

    async fn list_upcoming_events(
        &self,
        window: CalendarTimeRange,
        cursor: ProviderEventCursor,
    ) -> CalendarProviderResult<CalendarEventPage>;

    async fn event_details(
        &self,
        id: &CalendarEventId,
        occurrence_key: &EventOccurrenceKey,
    ) -> CalendarProviderResult<CalendarEvent>;
}
