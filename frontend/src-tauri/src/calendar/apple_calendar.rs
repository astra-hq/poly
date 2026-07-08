use crate::calendar::types::{CalendarPermissionProbe, CalendarPermissionStatus};
#[cfg(not(target_os = "macos"))]
use crate::calendar::types::{CalendarProbeError, CalendarProbeErrorKind};

#[tauri::command]
pub async fn check_calendar_permission() -> Result<CalendarPermissionProbe, String> {
    tokio::task::spawn_blocking(platform_probe)
        .await
        .map_err(|e| format!("Calendar permission probe task failed: {e}"))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn platform_probe() -> CalendarPermissionProbe {
    CalendarPermissionProbe::unavailable(
        CalendarPermissionStatus::UnsupportedPlatform,
        CalendarProbeError::new(
            CalendarProbeErrorKind::UnsupportedPlatform,
            "Apple Calendar EventKit probing is only available on macOS.",
        ),
    )
}

#[cfg(target_os = "macos")]
pub(crate) fn platform_probe() -> CalendarPermissionProbe {
    match crate::calendar::macos_eventkit::probe_eventkit() {
        Ok(probe) => probe,
        Err(error) => {
            CalendarPermissionProbe::unavailable(CalendarPermissionStatus::Unknown, error)
        }
    }
}
