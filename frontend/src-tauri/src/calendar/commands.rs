use tauri::State;

use crate::calendar::apple_provider::AppleCalendarProvider;
use crate::calendar::domain::{CalendarClock, CalendarInstant, CalendarTimeRange, SystemClock};
use crate::calendar::eligibility::auto_record_eligibility;
use crate::calendar::provider::CalendarProvider;
use crate::calendar::types::{CalendarPermissionStatus, CalendarProviderHealth};
use crate::poly_config::config::CalendarConfig;
use crate::poly_config::ConfigRepository;
use crate::state::AppState;

#[cfg(target_os = "macos")]
use crate::calendar::macos_eventkit;

pub fn get_calendar_settings_from_repo(repo: &ConfigRepository) -> anyhow::Result<CalendarConfig> {
    Ok(repo.load()?.calendar)
}

pub fn save_calendar_settings_to_repo(
    repo: &ConfigRepository,
    settings: CalendarConfig,
) -> anyhow::Result<CalendarConfig> {
    let mut config = repo.load()?;
    config.calendar = settings;
    repo.save_atomic(&config)?;
    Ok(config.calendar)
}

#[tauri::command]
pub async fn get_calendar_settings(state: State<'_, AppState>) -> Result<CalendarConfig, String> {
    get_calendar_settings_from_repo(&state.config_repo)
        .map_err(|e| format!("Failed to load calendar settings: {e}"))
}

#[tauri::command]
pub async fn save_calendar_settings(
    state: State<'_, AppState>,
    settings: CalendarConfig,
) -> Result<CalendarConfig, String> {
    save_calendar_settings_to_repo(&state.config_repo, settings)
        .map_err(|e| format!("Failed to save calendar settings: {e}"))
}

#[tauri::command]
pub async fn get_calendar_permission_status() -> Result<CalendarPermissionStatus, String> {
    platform_permission_status().map_err(|e| e)
}

#[tauri::command]
pub async fn request_calendar_permission() -> Result<CalendarPermissionStatus, String> {
    platform_request_permission().map_err(|e| e)
}

#[tauri::command]
pub async fn get_calendar_provider_health() -> Result<CalendarProviderHealth, String> {
    Ok(platform_provider_health())
}

#[tauri::command]
pub async fn get_upcoming_calendar_candidates(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let config = state
        .config_repo
        .load()
        .map_err(|e| format!("Failed to load config: {e}"))?;
    let calendar_config = config.calendar;

    let clock = SystemClock::default();
    let now = clock.now();
    let lookahead = chrono::Duration::minutes(calendar_config.lookahead_window_minutes as i64);
    let lookahead_end = CalendarInstant::from_utc(now.as_utc() + lookahead);

    let window = CalendarTimeRange::new(now, lookahead_end)
        .map_err(|e| format!("Invalid time range: {e}"))?;

    let provider = AppleCalendarProvider::default();
    let page = provider
        .list_upcoming_events(window, crate::calendar::domain::ProviderEventCursor::Start)
        .await
        .map_err(|e| format!("Failed to list calendar events: {e}"))?;

    let selected_ids: Option<Vec<String>> = {
        let ids = &calendar_config.selected_apple_calendar_identifiers;
        if ids.is_empty() {
            None
        } else {
            Some(ids.clone())
        }
    };

    let candidates: Vec<serde_json::Value> = page
        .events
        .into_iter()
        .filter(|event| {
            if let Some(ref ids) = selected_ids {
                ids.contains(&event.source.calendar_id)
            } else {
                true
            }
        })
        .map(|event| {
            let eligibility = auto_record_eligibility(&event, &clock);
            serde_json::json!({
                "id": event.id.as_str(),
                "occurrence_key": event.occurrence_key.as_str(),
                "title": event.details.title,
                "start": event.time_range.start.as_utc().to_rfc3339(),
                "end": event.time_range.end.as_utc().to_rfc3339(),
                "calendar_id": event.source.calendar_id,
                "meeting_link": event.details.meeting_link.map(|l| l.as_str().to_string()),
                "is_cancelled": event.is_cancelled,
                "category": format!("{:?}", event.category),
                "response_status": format!("{:?}", event.response_status),
                "eligible": eligibility.is_eligible(),
                "ineligibility_reason": match &eligibility {
                    crate::calendar::eligibility::AutoRecordEligibility::Eligible { .. } => serde_json::Value::Null,
                    crate::calendar::eligibility::AutoRecordEligibility::Ineligible { reason, .. } => {
                        serde_json::json!(format!("{:?}", reason))
                    }
                },
            })
        })
        .collect();

    Ok(serde_json::json!({"candidates": candidates}))
}

#[tauri::command]
pub async fn get_selected_calendars(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let config = state
        .config_repo
        .load()
        .map_err(|e| format!("Failed to load config: {e}"))?;
    Ok(config.calendar.selected_apple_calendar_identifiers)
}

#[cfg(not(target_os = "macos"))]
fn platform_permission_status() -> Result<CalendarPermissionStatus, String> {
    Ok(CalendarPermissionStatus::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
fn platform_permission_status() -> Result<CalendarPermissionStatus, String> {
    macos_eventkit::eventkit_authorization_status()
        .map_err(|e| format!("Failed to read calendar permission: {e:?}"))
}

#[cfg(not(target_os = "macos"))]
fn platform_request_permission() -> Result<CalendarPermissionStatus, String> {
    Ok(CalendarPermissionStatus::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
fn platform_request_permission() -> Result<CalendarPermissionStatus, String> {
    macos_eventkit::eventkit_request_access()
        .map_err(|e| format!("Failed to request calendar permission: {e:?}"))
}

#[cfg(not(target_os = "macos"))]
fn platform_provider_health() -> CalendarProviderHealth {
    CalendarProviderHealth::unsupported()
}

#[cfg(target_os = "macos")]
fn platform_provider_health() -> CalendarProviderHealth {
    match macos_eventkit::eventkit_authorization_status() {
        Ok(status) => {
            if !status.can_read_events() {
                return CalendarProviderHealth::error(
                    status,
                    "Calendar read access is not granted.",
                );
            }

            let now = chrono::Utc::now();
            let start = (now - chrono::Duration::days(30)).timestamp() as f64;
            let end = (now + chrono::Duration::days(1)).timestamp() as f64;

            match macos_eventkit::fetch_events(start, end) {
                Ok(events) => CalendarProviderHealth::ok(status, events.len()),
                Err(e) => CalendarProviderHealth::error(status, e.message),
            }
        }
        Err(e) => CalendarProviderHealth::error(CalendarPermissionStatus::Unknown, e.message),
    }
}
