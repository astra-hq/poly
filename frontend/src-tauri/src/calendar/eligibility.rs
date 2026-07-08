use crate::calendar::domain::{
    CalendarClock, CalendarEvent, CalendarEventId, EventCategory, EventOccurrenceKey,
    EventResponseStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoRecordIneligibilityReason {
    Declined,
    Cancelled,
    AllDay,
    MissingMeetingLink,
    AlreadyEnded,
    UnsupportedResponseStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoRecordEligibility {
    Eligible {
        id: CalendarEventId,
        occurrence_key: EventOccurrenceKey,
    },
    Ineligible {
        id: CalendarEventId,
        occurrence_key: EventOccurrenceKey,
        reason: AutoRecordIneligibilityReason,
    },
}

impl AutoRecordEligibility {
    pub const fn is_eligible(&self) -> bool {
        match self {
            Self::Eligible {
                id: _,
                occurrence_key: _,
            } => true,
            Self::Ineligible {
                id: _,
                occurrence_key: _,
                reason: _,
            } => false,
        }
    }

    pub fn eligible(id: CalendarEventId, occurrence_key: EventOccurrenceKey) -> Self {
        Self::Eligible { id, occurrence_key }
    }

    pub fn ineligible_declined(id: CalendarEventId, occurrence_key: EventOccurrenceKey) -> Self {
        Self::ineligible(id, occurrence_key, AutoRecordIneligibilityReason::Declined)
    }

    pub fn ineligible_cancelled(id: CalendarEventId, occurrence_key: EventOccurrenceKey) -> Self {
        Self::ineligible(id, occurrence_key, AutoRecordIneligibilityReason::Cancelled)
    }

    pub fn ineligible_all_day(id: CalendarEventId, occurrence_key: EventOccurrenceKey) -> Self {
        Self::ineligible(id, occurrence_key, AutoRecordIneligibilityReason::AllDay)
    }

    pub fn ineligible_missing_meeting_link(
        id: CalendarEventId,
        occurrence_key: EventOccurrenceKey,
    ) -> Self {
        Self::ineligible(
            id,
            occurrence_key,
            AutoRecordIneligibilityReason::MissingMeetingLink,
        )
    }

    fn ineligible(
        id: CalendarEventId,
        occurrence_key: EventOccurrenceKey,
        reason: AutoRecordIneligibilityReason,
    ) -> Self {
        Self::Ineligible {
            id,
            occurrence_key,
            reason,
        }
    }
}

pub fn auto_record_eligibility(
    event: &CalendarEvent,
    clock: &impl CalendarClock,
) -> AutoRecordEligibility {
    if event.is_cancelled {
        return AutoRecordEligibility::ineligible(
            event.id.clone(),
            event.occurrence_key.clone(),
            AutoRecordIneligibilityReason::Cancelled,
        );
    }

    match event.category {
        EventCategory::Timed => {}
        EventCategory::AllDay => {
            return AutoRecordEligibility::ineligible(
                event.id.clone(),
                event.occurrence_key.clone(),
                AutoRecordIneligibilityReason::AllDay,
            );
        }
        EventCategory::OutOfOffice
        | EventCategory::FocusTime
        | EventCategory::Reminder
        | EventCategory::Unknown => {
            return AutoRecordEligibility::ineligible(
                event.id.clone(),
                event.occurrence_key.clone(),
                AutoRecordIneligibilityReason::UnsupportedResponseStatus,
            );
        }
    }

    match event.response_status {
        EventResponseStatus::Accepted | EventResponseStatus::Tentative => {}
        EventResponseStatus::Declined => {
            return AutoRecordEligibility::ineligible(
                event.id.clone(),
                event.occurrence_key.clone(),
                AutoRecordIneligibilityReason::Declined,
            );
        }
        EventResponseStatus::NeedsAction
        | EventResponseStatus::Delegated
        | EventResponseStatus::Unknown => {
            return AutoRecordEligibility::ineligible(
                event.id.clone(),
                event.occurrence_key.clone(),
                AutoRecordIneligibilityReason::UnsupportedResponseStatus,
            );
        }
    }

    if event.details.meeting_link.is_none() {
        return AutoRecordEligibility::ineligible(
            event.id.clone(),
            event.occurrence_key.clone(),
            AutoRecordIneligibilityReason::MissingMeetingLink,
        );
    }

    if event.time_range.end <= clock.now() {
        return AutoRecordEligibility::ineligible(
            event.id.clone(),
            event.occurrence_key.clone(),
            AutoRecordIneligibilityReason::AlreadyEnded,
        );
    }

    AutoRecordEligibility::eligible(event.id.clone(), event.occurrence_key.clone())
}
