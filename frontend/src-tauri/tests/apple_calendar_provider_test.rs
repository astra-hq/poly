use app_lib::calendar::apple_provider::AppleCalendarProvider;
use app_lib::calendar::types::{CalendarPermissionStatus, CalendarProviderHealth};
use app_lib::poly_config::config::CalendarConfig;
use app_lib::poly_config::config::PolyConfig;
use app_lib::poly_config::ConfigRepository;

#[cfg(not(target_os = "macos"))]
use app_lib::calendar::domain::CalendarEventId;
#[cfg(not(target_os = "macos"))]
use app_lib::calendar::provider::CalendarProviderError;

fn temp_config_repo() -> (ConfigRepository, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("poly.yml");
    (ConfigRepository::with_path(path), dir)
}

#[test]
fn apple_provider_reports_correct_provider_kind() {
    use app_lib::calendar::provider::CalendarProvider;
    let provider = AppleCalendarProvider::default();
    assert_eq!(
        provider.provider_kind(),
        app_lib::calendar::domain::CalendarProviderKind::Apple
    );
}

#[test]
#[cfg(not(target_os = "macos"))]
fn apple_provider_returns_unsupported_platform_on_non_macos_listing() {
    use app_lib::calendar::provider::CalendarProvider;

    let rt = tokio::runtime::Runtime::new().unwrap();
    let provider = AppleCalendarProvider::default();
    let start = app_lib::calendar::domain::CalendarInstant::from_utc(chrono::Utc::now());
    let end = app_lib::calendar::domain::CalendarInstant::from_utc(
        chrono::Utc::now() + chrono::Duration::hours(1),
    );
    let window = app_lib::calendar::domain::CalendarTimeRange::new(start, end).unwrap();

    let result = rt.block_on(provider.list_upcoming_events(
        window,
        app_lib::calendar::domain::ProviderEventCursor::Start,
    ));

    assert_eq!(
        result,
        Err(CalendarProviderError::UnsupportedPlatform {
            provider_kind: app_lib::calendar::domain::CalendarProviderKind::Apple,
        })
    );
}

#[test]
#[cfg(not(target_os = "macos"))]
fn apple_provider_returns_unsupported_platform_on_non_macos_details() {
    use app_lib::calendar::provider::CalendarProvider;

    let rt = tokio::runtime::Runtime::new().unwrap();
    let provider = AppleCalendarProvider::default();

    let result = rt.block_on(provider.event_details(
        &CalendarEventId::new("test-id"),
        &app_lib::calendar::domain::EventOccurrenceKey::new("test-key"),
    ));

    assert_eq!(
        result,
        Err(CalendarProviderError::UnsupportedPlatform {
            provider_kind: app_lib::calendar::domain::CalendarProviderKind::Apple,
        })
    );
}

#[test]
#[cfg(target_os = "macos")]
fn apple_provider_non_macos_tests_are_cfg_disabled_on_macos() {
    assert!(cfg!(target_os = "macos"));
}

#[test]
fn provider_health_unsupported_has_correct_fields() {
    let health = CalendarProviderHealth::unsupported();
    assert_eq!(health.provider, "apple");
    assert!(!health.platform_supported);
    assert!(!health.permission_granted);
    assert_eq!(
        health.permission_status,
        CalendarPermissionStatus::UnsupportedPlatform
    );
    assert!(health.event_count.is_none());
    assert!(health.error.is_some());
}

#[test]
fn provider_health_ok_has_correct_fields() {
    let health = CalendarProviderHealth::ok(CalendarPermissionStatus::FullAccess, 42);
    assert_eq!(health.provider, "apple");
    assert!(health.platform_supported);
    assert!(health.permission_granted);
    assert_eq!(
        health.permission_status,
        CalendarPermissionStatus::FullAccess
    );
    assert_eq!(health.event_count, Some(42));
    assert!(health.error.is_none());
}

#[test]
fn provider_health_error_has_correct_fields() {
    let health =
        CalendarProviderHealth::error(CalendarPermissionStatus::Denied, "test error message");
    assert_eq!(health.provider, "apple");
    assert!(!health.permission_granted);
    assert_eq!(health.permission_status, CalendarPermissionStatus::Denied);
    assert!(health.event_count.is_none());
    assert_eq!(health.error.as_deref(), Some("test error message"));
}

#[test]
#[cfg(not(target_os = "macos"))]
fn get_calendar_permission_status_returns_unsupported_on_non_macos() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(app_lib::calendar::commands::get_calendar_permission_status());
    assert_eq!(result, Ok(CalendarPermissionStatus::UnsupportedPlatform));
}

#[test]
#[cfg(not(target_os = "macos"))]
fn request_calendar_permission_returns_unsupported_on_non_macos() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(app_lib::calendar::commands::request_calendar_permission());
    assert_eq!(result, Ok(CalendarPermissionStatus::UnsupportedPlatform));
}

#[test]
fn get_selected_calendars_returns_empty_when_none_selected() {
    let (repo, _dir) = temp_config_repo();
    repo.save_atomic(&PolyConfig::default()).unwrap();

    let settings = app_lib::calendar::commands::get_calendar_settings_from_repo(&repo).unwrap();
    assert!(settings.selected_apple_calendar_identifiers.is_empty());
}

#[test]
fn get_selected_calendars_returns_saved_identifiers() {
    let (repo, _dir) = temp_config_repo();
    let cfg = PolyConfig {
        calendar: CalendarConfig {
            selected_apple_calendar_identifiers: vec![
                "calendar://work".to_string(),
                "calendar://personal".to_string(),
            ],
            ..CalendarConfig::default()
        },
        ..PolyConfig::default()
    };
    repo.save_atomic(&cfg).unwrap();

    let settings = app_lib::calendar::commands::get_calendar_settings_from_repo(&repo).unwrap();
    assert_eq!(
        settings.selected_apple_calendar_identifiers,
        vec!["calendar://work", "calendar://personal"]
    );
}

#[test]
#[cfg(not(target_os = "macos"))]
fn get_calendar_provider_health_returns_unsupported_on_non_macos() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(app_lib::calendar::commands::get_calendar_provider_health());
    let health = result.unwrap();
    assert!(!health.platform_supported);
    assert_eq!(
        health.permission_status,
        CalendarPermissionStatus::UnsupportedPlatform
    );
}

#[test]
#[cfg(target_os = "macos")]
fn get_calendar_provider_health_non_macos_tests_are_cfg_disabled() {
    assert!(cfg!(target_os = "macos"));
}

#[test]
fn calendar_config_default_has_apple_provider() {
    let cfg = CalendarConfig::default();
    assert_eq!(
        cfg.provider,
        app_lib::poly_config::config::CalendarProvider::Apple
    );
    assert!(!cfg.auto_record_enabled);
    assert!(cfg.metadata_pull_enabled);
    assert_eq!(cfg.lookahead_window_minutes, 60);
    assert_eq!(cfg.start_grace_window_minutes, 5);
    assert_eq!(cfg.end_grace_window_minutes, 5);
    assert!(cfg.selected_apple_calendar_identifiers.is_empty());
}
