use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use log::{error, info, warn};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::RwLock;
use tokio::time;

use crate::calendar::apple_provider::AppleCalendarProvider;
use crate::calendar::domain::{
    CalendarClock, CalendarEvent, CalendarEventId, CalendarInstant, CalendarTimeRange,
    EventOccurrenceKey, ProviderEventCursor, SystemClock,
};
use crate::calendar::eligibility::auto_record_eligibility;
use crate::calendar::provider::CalendarProvider;
use crate::calendar::recording_metadata::CalendarRecordingContext;
use crate::notifications::commands::NotificationManagerState;
use crate::poly_config::config::CalendarConfig;
use crate::poly_config::ConfigRepository;

/// Dedupe key: `"{provider_kind}:{event_id}:{occurrence_start_rfc3339}"`
type DedupeKey = String;

/// Outcome of a scheduler tick — computed by `compute_scheduler_action`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerAction {
    /// Pop this meeting's cork.
    StartRecording {
        event_id: CalendarEventId,
        occurrence_key: EventOccurrenceKey,
        event_title: String,
        start_utc: DateTime<Utc>,
        end_utc: DateTime<Utc>,
        dedupe_key: DedupeKey,
    },
    /// No eligible meetings in the lookahead window.
    SkipNoCandidates,
    /// An eligible meeting exists, but recording is already in-flight.
    SkipRecordingActive {
        event_id: CalendarEventId,
        event_title: String,
    },
    /// The best candidate is outside its grace window.
    SkipOutsideGraceWindow {
        event_id: CalendarEventId,
        event_title: String,
    },
    /// All candidates were already deduped.
    SkipAllDeduped,
}

/// Execution context passed to the pure `compute_scheduler_action`.
pub struct SchedulerTickInput {
    /// The current wall-clock time.
    pub now: CalendarInstant,
    /// The live calendar config.
    pub config: CalendarConfig,
    /// Events returned by the provider for the lookahead window.
    pub events: Vec<CalendarEvent>,
    /// Whether a recording session is currently running.
    pub is_recording: bool,
    /// Previously-recorded dedupe keys.
    pub dedupe: HashSet<DedupeKey>,
}

/// Pure function — no I/O, no side-effects.  Test the scheduler's brain here.
pub fn compute_scheduler_action(input: &SchedulerTickInput) -> SchedulerAction {
    if !input.config.auto_record_enabled {
        return SchedulerAction::SkipNoCandidates;
    }

    let selected_ids: Option<&Vec<String>> = {
        let ids = &input.config.selected_apple_calendar_identifiers;
        if ids.is_empty() {
            None
        } else {
            Some(ids)
        }
    };

    let clock = SystemClock; // use system clock for eligibility — input.now is for grace windows

    // Collect eligible + calendar-filtered events, sorted by start time.
    let mut candidates: Vec<&CalendarEvent> = input
        .events
        .iter()
        .filter(|event| {
            if let Some(ids) = selected_ids {
                ids.contains(&event.source.calendar_id)
            } else {
                true
            }
        })
        .filter(|event| auto_record_eligibility(event, &clock).is_eligible())
        .collect();

    candidates.sort_by_key(|e| e.time_range.start);

    if candidates.is_empty() {
        return SchedulerAction::SkipNoCandidates;
    }

    let now_dt = input.now.as_utc();
    let start_grace = Duration::minutes(input.config.start_grace_window_minutes as i64);
    let end_grace = Duration::minutes(input.config.end_grace_window_minutes as i64);

    // Find the first candidate within its grace window.
    for event in &candidates {
        let event_start = event.time_range.start.as_utc();
        let grace_start = event_start - start_grace;
        let grace_end = event_start + end_grace;

        if now_dt < grace_start || now_dt > grace_end {
            continue; // outside grace window — try next candidate
        }

        let dedupe_key = make_dedupe_key(event);

        if input.dedupe.contains(&dedupe_key) {
            continue; // already recorded — try next candidate
        }

        if input.is_recording {
            return SchedulerAction::SkipRecordingActive {
                event_id: event.id.clone(),
                event_title: event.details.title.clone(),
            };
        }

        return SchedulerAction::StartRecording {
            event_id: event.id.clone(),
            occurrence_key: event.occurrence_key.clone(),
            event_title: event.details.title.clone(),
            start_utc: event.time_range.start.as_utc(),
            end_utc: event.time_range.end.as_utc(),
            dedupe_key,
        };
    }

    // All candidates are either out of grace window or deduped.
    let first = candidates.first().unwrap();
    let is_deduped = input.dedupe.contains(&make_dedupe_key(first));
    if is_deduped {
        SchedulerAction::SkipAllDeduped
    } else {
        SchedulerAction::SkipOutsideGraceWindow {
            event_id: first.id.clone(),
            event_title: first.details.title.clone(),
        }
    }
}

fn make_dedupe_key(event: &CalendarEvent) -> DedupeKey {
    format!(
        "{:?}:{}:{}",
        event.source.provider_kind,
        event.id.as_str(),
        event.time_range.start.as_utc().to_rfc3339()
    )
}

/// Serializable payload emitted to the frontend on each scheduler tick.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchedulerStatusEvent {
    CandidateFound {
        event_id: String,
        title: String,
        start: String,
        end: String,
    },
    RecordingStarted {
        event_id: String,
        title: String,
        start: String,
    },
    RecordingSkippedActive {
        event_id: String,
        title: String,
    },
    Skipped {
        reason: String,
    },
    Error {
        message: String,
    },
    Stopped,
}

/// The live scheduler service.  Owned by the Tauri `setup` hook; runs a
/// background tokio task polling the calendar provider.
pub struct CalendarRecordingScheduler {
    dedupe: Arc<RwLock<HashSet<DedupeKey>>>,
    config_repo: Arc<ConfigRepository>,
}

impl CalendarRecordingScheduler {
    pub fn new(config_repo: Arc<ConfigRepository>) -> Self {
        Self {
            dedupe: Arc::new(RwLock::new(HashSet::new())),
            config_repo,
        }
    }

    /// Spawn the polling loop.  Returns `true` if a scheduler was started,
    /// `false` if auto-record is disabled in the current config.
    pub async fn start<R: Runtime>(self: Arc<Self>, app: AppHandle<R>) {
        let config = match self.config_repo.load() {
            Ok(c) => c,
            Err(e) => {
                error!("Calendar scheduler cannot load config: {e}");
                return;
            }
        };

        if !config.calendar.auto_record_enabled {
            info!("Calendar auto-record is disabled; scheduler will not start.");
            return;
        }

        let interval_secs = config.calendar.poll_interval_seconds.max(10) as u64;

        info!(
            "Calendar auto-record scheduler started (poll every {}s, lookahead {}m, grace {}m before / {}m after)",
            interval_secs,
            config.calendar.lookahead_window_minutes,
            config.calendar.start_grace_window_minutes,
            config.calendar.end_grace_window_minutes,
        );

        let mut ticker = time::interval(time::Duration::from_secs(interval_secs));
        // Skip the immediate first tick so the app has time to settle.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            self.tick(&app).await;
        }
    }

    async fn tick<R: Runtime>(&self, app: &AppHandle<R>) {
        let config = match self.config_repo.load() {
            Ok(c) => c,
            Err(e) => {
                error!("Calendar scheduler failed to reload config: {e}");
                return;
            }
        };

        if !config.calendar.auto_record_enabled {
            perf_debug!("Calendar scheduler: auto_record disabled, skipping tick");
            return;
        }

        let clock = SystemClock::default();
        let now = clock.now();
        let lookahead = Duration::minutes(config.calendar.lookahead_window_minutes as i64);
        let lookahead_end = CalendarInstant::from_utc(now.as_utc() + lookahead);

        let window = match CalendarTimeRange::new(now, lookahead_end) {
            Ok(w) => w,
            Err(e) => {
                error!("Calendar scheduler: invalid time range: {e}");
                return;
            }
        };

        let provider = AppleCalendarProvider::default();
        let page = match provider
            .list_upcoming_events(window, ProviderEventCursor::Start)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                warn!("Calendar scheduler: provider returned error: {e}");
                let _ = app.emit(
                    "calendar-scheduler-status",
                    SchedulerStatusEvent::Error {
                        message: format!("Provider error: {e}"),
                    },
                );
                let manager_state: State<'_, NotificationManagerState<R>> = app.state();
                let _ = crate::notifications::commands::show_calendar_scheduler_error_notification(
                    app,
                    &manager_state,
                    format!("Provider error: {e}"),
                )
                .await;
                return;
            }
        };

        let is_recording = crate::audio::recording_commands::is_recording().await;

        let input = SchedulerTickInput {
            now,
            config: config.calendar.clone(),
            events: page.events,
            is_recording,
            dedupe: self.dedupe.read().await.clone(),
        };

        let action = compute_scheduler_action(&input);

        match action {
            SchedulerAction::StartRecording {
                event_id,
                occurrence_key: _,
                event_title,
                start_utc,
                end_utc,
                dedupe_key,
            } => {
                info!(
                    "Calendar scheduler: auto-starting recording for \"{}\" at {}",
                    event_title, start_utc
                );

                // Persist dedupe state BEFORE starting recording — prevents
                // double-start if another tick fires during recording init.
                {
                    self.dedupe.write().await.insert(dedupe_key.clone());
                }

                let context = CalendarRecordingContext {
                    provider_kind: format!("{:?}", config.calendar.provider).to_lowercase(),
                    event_id: event_id.as_str().to_string(),
                    occurrence_start: start_utc.to_rfc3339(),
                    occurrence_end: end_utc.to_rfc3339(),
                    event_title: event_title.clone(),
                    metadata_status: "enriched".to_string(),
                };

                match crate::audio::recording_commands::start_recording_with_devices_and_meeting(
                    app.clone(),
                    None,
                    None,
                    Some(event_title.clone()),
                    Some(context),
                )
                .await
                {
                    Ok(()) => {
                        let _ = app.emit(
                            "calendar-scheduler-status",
                            SchedulerStatusEvent::RecordingStarted {
                                event_id: event_id.as_str().to_string(),
                                title: event_title.clone(),
                                start: start_utc.to_rfc3339(),
                            },
                        );
                        let manager_state: State<'_, NotificationManagerState<R>> = app.state();
                        let _ = crate::notifications::commands::show_calendar_auto_record_started_notification(
                            app,
                            &manager_state,
                        )
                        .await;
                    }
                    Err(e) => {
                        error!("Calendar scheduler failed to start recording: {e}");
                        self.dedupe.write().await.remove(&dedupe_key);
                        let _ = app.emit(
                            "calendar-scheduler-status",
                            SchedulerStatusEvent::Error {
                                message: format!("Failed to start recording: {e}"),
                            },
                        );
                        let manager_state: State<'_, NotificationManagerState<R>> = app.state();
                        let _ = crate::notifications::commands::show_calendar_scheduler_error_notification(
                            app,
                            &manager_state,
                            format!("Failed to start recording: {e}"),
                        )
                        .await;
                    }
                }
            }

            SchedulerAction::SkipNoCandidates => {
                perf_debug!("Calendar scheduler: no eligible candidates in lookahead window");
            }

            SchedulerAction::SkipRecordingActive {
                event_id,
                event_title,
            } => {
                perf_debug!(
                    "Calendar scheduler: recording already active, skipping \"{}\"",
                    event_title
                );
                let _ = app.emit(
                    "calendar-scheduler-status",
                    SchedulerStatusEvent::RecordingSkippedActive {
                        event_id: event_id.as_str().to_string(),
                        title: event_title.clone(),
                    },
                );
                let manager_state: State<'_, NotificationManagerState<R>> = app.state();
                let _ = crate::notifications::commands::show_calendar_auto_record_skipped_notification(
                    app,
                    &manager_state,
                    "A recording is already in progress. The scheduled meeting will not be recorded automatically.",
                )
                .await;
            }

            SchedulerAction::SkipOutsideGraceWindow {
                event_id,
                event_title,
            } => {
                perf_debug!(
                    "Calendar scheduler: \"{}\" is outside grace window",
                    event_title
                );
                let _ = app.emit(
                    "calendar-scheduler-status",
                    SchedulerStatusEvent::CandidateFound {
                        event_id: event_id.as_str().to_string(),
                        title: event_title,
                        start: "".to_string(),
                        end: "".to_string(),
                    },
                );
            }

            SchedulerAction::SkipAllDeduped => {
                perf_debug!("Calendar scheduler: all candidates already deduped");
            }
        }
    }

    /// Stop the scheduler (signal cancellation).
    /// In the current design the polling loop runs indefinitely; restart requires
    /// a new scheduler instance which the caller creates in `setup`.
    pub fn stop(&self) {
        // No-op: the scheduler loop is unbounded; a future enhancement
        // can add a CancellationToken for graceful shutdown.
        info!("Calendar scheduler stop requested (no-op in current design)");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::domain::{
        CalendarAttendee, CalendarEventBody, CalendarEventDetails, CalendarEventSource,
        CalendarMeetingLink, CalendarOrganizer, CalendarProviderKind, EventCategory,
        EventResponseStatus,
    };
    use crate::calendar::eligibility::AutoRecordEligibility;

    // ── helpers ───────────────────────────────────────────────────────────

    fn event(
        id: &str,
        title: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        meeting_link: Option<&str>,
    ) -> CalendarEvent {
        CalendarEvent {
            source: CalendarEventSource::new(CalendarProviderKind::Apple, "cal-1"),
            id: CalendarEventId::new(id),
            occurrence_key: EventOccurrenceKey::new(format!("{id}|{}", start.timestamp())),
            details: CalendarEventDetails {
                title: title.to_string(),
                body: Some(CalendarEventBody::new("Agenda")),
                location: None,
                meeting_link: meeting_link.map(|l| CalendarMeetingLink::new(l.to_string())),
                organizer: Some(CalendarOrganizer::new("a@b.com", Some("Alice".into()))),
                attendees: vec![CalendarAttendee::new(
                    "b@b.com",
                    Some("Bob".into()),
                    EventResponseStatus::Accepted,
                )],
            },
            time_range: CalendarTimeRange::new(
                CalendarInstant::from_utc(start),
                CalendarInstant::from_utc(end),
            )
            .unwrap(),
            response_status: EventResponseStatus::Accepted,
            category: EventCategory::Timed,
            is_cancelled: false,
        }
    }

    fn make_input(
        now: DateTime<Utc>,
        events: Vec<CalendarEvent>,
        is_recording: bool,
        dedupe: HashSet<DedupeKey>,
    ) -> SchedulerTickInput {
        let mut config = CalendarConfig::default();
        config.auto_record_enabled = true;
        config.lookahead_window_minutes = 60;
        config.start_grace_window_minutes = 5;
        config.end_grace_window_minutes = 5;

        SchedulerTickInput {
            now: CalendarInstant::from_utc(now),
            config,
            events,
            is_recording,
            dedupe,
        }
    }

    // ── tests ─────────────────────────────────────────────────────────────

    #[test]
    fn exactly_once_start_within_grace_window() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let evt = event(
            "evt-1",
            "Standup",
            start,
            end,
            Some("https://zoom.us/j/123"),
        );

        let input = make_input(start, vec![evt.clone()], false, HashSet::new());

        match compute_scheduler_action(&input) {
            SchedulerAction::StartRecording {
                event_title,
                dedupe_key,
                ..
            } => {
                assert_eq!(event_title, "Standup");
                assert!(dedupe_key.contains("evt-1"));
            }
            other => panic!("Expected StartRecording, got {other:?}"),
        }
    }

    #[test]
    fn skip_outside_grace_window() {
        // Meeting starts 20 minutes from now, grace window is 5 min before.
        let start = Utc::now() + Duration::minutes(20);
        let end = start + Duration::minutes(30);
        let evt = event("evt-2", "Review", start, end, Some("https://zoom.us/j/456"));

        let input = make_input(Utc::now(), vec![evt.clone()], false, HashSet::new());

        match compute_scheduler_action(&input) {
            SchedulerAction::SkipOutsideGraceWindow { event_title, .. } => {
                assert_eq!(event_title, "Review");
            }
            other => panic!("Expected SkipOutsideGraceWindow, got {other:?}"),
        }
    }

    #[test]
    fn skip_when_recording_active() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let evt = event("evt-3", "Sync", start, end, Some("https://zoom.us/j/789"));

        let input = make_input(start, vec![evt.clone()], true, HashSet::new());

        match compute_scheduler_action(&input) {
            SchedulerAction::SkipRecordingActive { event_title, .. } => {
                assert_eq!(event_title, "Sync");
            }
            other => panic!("Expected SkipRecordingActive, got {other:?}"),
        }
    }

    #[test]
    fn dedupe_persistence() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let evt = event(
            "evt-4",
            "Planning",
            start,
            end,
            Some("https://zoom.us/j/abc"),
        );

        let mut dedupe = HashSet::new();
        dedupe.insert(make_dedupe_key(&evt));

        let input = make_input(start, vec![evt.clone()], false, dedupe);

        match compute_scheduler_action(&input) {
            SchedulerAction::SkipAllDeduped => {} // expected
            other => panic!("Expected SkipAllDeduped, got {other:?}"),
        }
    }

    #[test]
    fn overlap_picks_earliest_eligible() {
        // Two meetings starting at the same time — picks the first (sorted by start).
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let evt_a = event("evt-a", "Alpha", start, end, Some("https://zoom.us/j/111"));
        let evt_b = event("evt-b", "Beta", start, end, Some("https://zoom.us/j/222"));

        let input = make_input(
            start,
            vec![evt_a.clone(), evt_b.clone()],
            false,
            HashSet::new(),
        );

        match compute_scheduler_action(&input) {
            SchedulerAction::StartRecording { event_title, .. } => {
                assert_eq!(event_title, "Alpha"); // sorted first by start time
            }
            other => panic!("Expected StartRecording, got {other:?}"),
        }
    }

    #[test]
    fn no_candidates_when_auto_record_disabled() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let evt = event(
            "evt-6",
            "Skipped",
            start,
            end,
            Some("https://zoom.us/j/xyz"),
        );

        let mut input = make_input(start, vec![evt.clone()], false, HashSet::new());
        input.config.auto_record_enabled = false;

        assert_eq!(
            compute_scheduler_action(&input),
            SchedulerAction::SkipNoCandidates
        );
    }

    #[test]
    fn skip_ineligible_missing_link() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        // No meeting link → ineligible.
        let evt = event("evt-7", "No Link", start, end, None);

        let input = make_input(start, vec![evt.clone()], false, HashSet::new());

        assert_eq!(
            compute_scheduler_action(&input),
            SchedulerAction::SkipNoCandidates
        );
    }

    #[test]
    fn skip_declined_even_within_grace() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let mut evt = event(
            "evt-8",
            "Declined",
            start,
            end,
            Some("https://zoom.us/j/999"),
        );
        evt.response_status = EventResponseStatus::Declined;

        let input = make_input(start, vec![evt.clone()], false, HashSet::new());

        assert_eq!(
            compute_scheduler_action(&input),
            SchedulerAction::SkipNoCandidates
        );
    }

    #[test]
    fn dedupe_key_is_deterministic() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let evt = event(
            "evt-d",
            "KeyTest",
            start,
            end,
            Some("https://zoom.us/j/key"),
        );

        let k1 = make_dedupe_key(&evt);
        let k2 = make_dedupe_key(&evt);

        assert_eq!(k1, k2);
    }

    #[test]
    fn dedupe_key_differs_by_event_id() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let evt_a = event("evt-1", "A", start, end, Some("https://zoom.us/j/a"));
        let evt_b = event("evt-2", "B", start, end, Some("https://zoom.us/j/b"));

        assert_ne!(make_dedupe_key(&evt_a), make_dedupe_key(&evt_b));
    }

    #[test]
    fn dedupe_key_differs_by_start_time() {
        let start_a = Utc::now();
        let start_b = start_a + Duration::hours(1);
        let end_a = start_a + Duration::minutes(30);
        let end_b = start_b + Duration::minutes(30);
        let evt_a = event("evt-1", "A", start_a, end_a, Some("https://zoom.us/j/a"));
        let evt_b = event("evt-1", "A", start_b, end_b, Some("https://zoom.us/j/a"));

        assert_ne!(make_dedupe_key(&evt_a), make_dedupe_key(&evt_b));
    }

    #[test]
    fn next_candidate_selected_after_first_is_deduped() {
        let now = Utc::now();
        let start_a = now + Duration::minutes(1); // within grace
        let end_a = start_a + Duration::minutes(30);
        let start_b = now + Duration::minutes(2); // also within grace
        let end_b = start_b + Duration::minutes(30);

        let evt_a = event(
            "evt-a",
            "Alpha",
            start_a,
            end_a,
            Some("https://zoom.us/j/a"),
        );
        let evt_b = event("evt-b", "Beta", start_b, end_b, Some("https://zoom.us/j/b"));

        let mut dedupe = HashSet::new();
        dedupe.insert(make_dedupe_key(&evt_a));

        let input = make_input(now, vec![evt_a.clone(), evt_b.clone()], false, dedupe);

        match compute_scheduler_action(&input) {
            SchedulerAction::StartRecording { event_title, .. } => {
                // Should skip deduped "Alpha" and pick "Beta"
                assert_eq!(event_title, "Beta");
            }
            other => panic!("Expected StartRecording(Beta), got {other:?}"),
        }
    }

    #[test]
    fn skipped_when_outside_end_grace() {
        // Meeting started 10 minutes ago, grace window is 5 min after start → outside.
        let start = Utc::now() - Duration::minutes(10);
        let end = start + Duration::minutes(30);
        let evt = event("evt-10", "Late", start, end, Some("https://zoom.us/j/late"));

        let input = make_input(Utc::now(), vec![evt.clone()], false, HashSet::new());

        match compute_scheduler_action(&input) {
            SchedulerAction::SkipOutsideGraceWindow { event_title, .. } => {
                assert_eq!(event_title, "Late");
            }
            other => panic!("Expected SkipOutsideGraceWindow, got {other:?}"),
        }
    }

    #[test]
    fn selected_calendar_filtering() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        let mut evt = event(
            "evt-11",
            "Filtered",
            start,
            end,
            Some("https://zoom.us/j/f"),
        );
        evt.source = CalendarEventSource::new(CalendarProviderKind::Apple, "different-calendar");

        let mut input = make_input(start, vec![evt.clone()], false, HashSet::new());
        input.config.selected_apple_calendar_identifiers = vec!["cal-1".to_string()];

        // event has calendar_id "different-calendar", but we only accept "cal-1"
        assert_eq!(
            compute_scheduler_action(&input),
            SchedulerAction::SkipNoCandidates
        );
    }
}
