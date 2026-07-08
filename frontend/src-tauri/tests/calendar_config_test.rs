use app_lib::calendar::commands::{
    get_calendar_settings_from_repo, save_calendar_settings_to_repo,
};
use app_lib::poly_config::config::{CalendarConfig, CalendarProvider, PolyConfig};
use app_lib::poly_config::ConfigRepository;

fn temp_config_repo() -> (ConfigRepository, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("poly.yml");
    (ConfigRepository::with_path(path), dir)
}

#[test]
fn calendar_config_serializes_non_secret_settings_when_customized() {
    // Given: calendar settings containing Apple calendar identifiers but no secrets.
    let cfg = PolyConfig {
        calendar: CalendarConfig {
            metadata_pull_enabled: true,
            auto_record_enabled: true,
            provider: CalendarProvider::Apple,
            lookahead_window_minutes: 90,
            start_grace_window_minutes: 7,
            end_grace_window_minutes: 12,
            selected_apple_calendar_identifiers: vec![
                "calendar://work".to_string(),
                "calendar://personal".to_string(),
            ],
            show_calendar_status: true,
            show_next_meeting_banner: false,
            poll_interval_seconds: 30,
        },
        ..PolyConfig::default()
    };

    // When: serializing to the durable YAML format.
    let yaml = serde_yaml::to_string(&cfg).unwrap();

    // Then: calendar settings are present and secret-shaped fields are absent.
    assert!(yaml.contains("calendar:"));
    assert!(yaml.contains("provider: apple"));
    assert!(yaml.contains("auto_record_enabled: true"));
    assert!(yaml.contains("metadata_pull_enabled: true"));
    assert!(yaml.contains("lookahead_window_minutes: 90"));
    assert!(yaml.contains("start_grace_window_minutes: 7"));
    assert!(yaml.contains("end_grace_window_minutes: 12"));
    assert!(yaml.contains("calendar://work"));

    let lower = yaml.to_lowercase();
    assert!(
        !lower.contains("access_token"),
        "calendar token leaked into YAML"
    );
    assert!(
        !lower.contains("refresh_token"),
        "calendar token leaked into YAML"
    );
    assert!(!lower.contains("oauth"), "OAuth data leaked into YAML");
    assert!(!lower.contains("api_key"), "API key leaked into YAML");
}

#[test]
fn calendar_config_defaults_when_missing_from_legacy_yaml() {
    // Given: older YAML with no calendar section.
    let yaml = "summary:\n  provider_id: local\n";

    // When: loading the config.
    let cfg = PolyConfig::load_from_str(yaml).unwrap();

    // Then: calendar settings are defaulted for Apple-only v1 behavior.
    assert_eq!(cfg.calendar, CalendarConfig::default());
    assert_eq!(cfg.calendar.provider, CalendarProvider::Apple);
    assert!(!cfg.calendar.auto_record_enabled);
    assert!(cfg.calendar.metadata_pull_enabled);
    assert_eq!(cfg.calendar.lookahead_window_minutes, 60);
    assert_eq!(cfg.calendar.start_grace_window_minutes, 5);
    assert_eq!(cfg.calendar.end_grace_window_minutes, 5);
    assert!(cfg.calendar.selected_apple_calendar_identifiers.is_empty());
}

#[test]
fn calendar_config_settings_roundtrip_uses_config_repository_atomic_save() {
    // Given: a repository with an existing config and customized calendar settings.
    let (repo, _dir) = temp_config_repo();
    repo.save_atomic(&PolyConfig::default()).unwrap();
    let settings = CalendarConfig {
        auto_record_enabled: true,
        lookahead_window_minutes: 120,
        selected_apple_calendar_identifiers: vec!["calendar://engineering".to_string()],
        show_calendar_status: false,
        ..CalendarConfig::default()
    };

    // When: saving through the calendar command helper.
    let saved = save_calendar_settings_to_repo(&repo, settings.clone()).unwrap();

    // Then: the returned and reloaded settings match, persisted through poly.yml.
    assert_eq!(saved, settings);
    let loaded = get_calendar_settings_from_repo(&repo).unwrap();
    assert_eq!(loaded, settings);

    let yaml = std::fs::read_to_string(repo.path()).unwrap();
    assert!(yaml.contains("calendar:"));
    assert!(yaml.contains("calendar://engineering"));
}
