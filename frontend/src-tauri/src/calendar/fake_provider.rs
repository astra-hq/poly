use async_trait::async_trait;

use crate::calendar::domain::{
    CalendarClock, CalendarEvent, CalendarEventId, CalendarEventPage, CalendarInstant,
    CalendarProviderKind, CalendarTimeRange, EventOccurrenceKey, ProviderEventCursor,
};
use crate::calendar::provider::{CalendarProvider, CalendarProviderError, CalendarProviderResult};

#[derive(Debug, Clone)]
pub struct FakeClock {
    now: CalendarInstant,
}

impl FakeClock {
    pub const fn new(now: CalendarInstant) -> Self {
        Self { now }
    }
}

impl CalendarClock for FakeClock {
    fn now(&self) -> CalendarInstant {
        self.now
    }
}

#[derive(Debug, Clone)]
pub struct FakeCalendarProvider {
    events: Vec<CalendarEvent>,
}

impl FakeCalendarProvider {
    pub fn new(events: Vec<CalendarEvent>) -> Self {
        Self { events }
    }
}

#[async_trait]
impl CalendarProvider for FakeCalendarProvider {
    fn provider_kind(&self) -> CalendarProviderKind {
        CalendarProviderKind::Fake
    }

    async fn list_upcoming_events(
        &self,
        window: CalendarTimeRange,
        cursor: ProviderEventCursor,
    ) -> CalendarProviderResult<CalendarEventPage> {
        match cursor {
            ProviderEventCursor::Start => {
                let events = self
                    .events
                    .iter()
                    .filter(|event| event.time_range.overlaps(&window))
                    .cloned()
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

    async fn event_details(
        &self,
        id: &CalendarEventId,
        occurrence_key: &EventOccurrenceKey,
    ) -> CalendarProviderResult<CalendarEvent> {
        self.events
            .iter()
            .find(|event| &event.id == id && &event.occurrence_key == occurrence_key)
            .cloned()
            .ok_or_else(|| CalendarProviderError::EventNotFound {
                id: id.clone(),
                occurrence_key: occurrence_key.clone(),
            })
    }
}
