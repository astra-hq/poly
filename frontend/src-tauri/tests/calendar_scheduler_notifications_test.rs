use app_lib::notifications::{
    settings::NotificationPreferences,
    types::{Notification, NotificationType},
};

#[test]
fn calendar_notification_types_exist() {
    let started = Notification::calendar_auto_record_started();
    assert_eq!(started.title, "Poly");
    assert!(!started.body.is_empty());
    assert!(
        matches!(
            started.notification_type,
            NotificationType::CalendarAutoRecordStarted
        ),
        "Expected CalendarAutoRecordStarted variant"
    );

    let skipped = Notification::calendar_auto_record_skipped("test reason");
    assert_eq!(skipped.title, "Poly");
    assert_eq!(skipped.body, "test reason");
    assert!(
        matches!(
            skipped.notification_type,
            NotificationType::CalendarAutoRecordSkipped
        ),
        "Expected CalendarAutoRecordSkipped variant"
    );

    let error = Notification::calendar_scheduler_error("test error");
    assert_eq!(error.title, "Poly Error");
    assert_eq!(error.body, "test error");
    assert!(
        matches!(
            error.notification_type,
            NotificationType::CalendarSchedulerError
        ),
        "Expected CalendarSchedulerError variant"
    );
}

#[test]
fn calendar_auto_record_started_body_is_privacy_safe() {
    let notification = Notification::calendar_auto_record_started();
    let body = notification.body.to_lowercase();

    assert!(!body.contains("meeting title"));
    assert!(!body.contains("standup"));
    assert!(!body.contains("board"));
    assert!(!body.contains("zoom"));
    assert!(!body.contains("https://"));
    assert!(!body.contains("@"));
    assert!(!body.contains("attendee"));
    assert!(!body.contains("organizer"));
}

#[test]
fn calendar_auto_record_skipped_body_is_privacy_safe() {
    let notification = Notification::calendar_auto_record_skipped(
        "A recording is already in progress. The scheduled meeting will not be recorded automatically.",
    );
    let body = notification.body.to_lowercase();

    assert!(!body.contains("secret"));
    assert!(!body.contains("https://"));
    assert!(!body.contains("@"));
    assert!(!body.contains("zoom.us"));
}

#[test]
fn calendar_scheduler_error_body_is_privacy_safe() {
    let notification = Notification::calendar_scheduler_error("Provider error: connection failed");
    let body = notification.body.to_lowercase();

    assert!(!body.contains("https://"));
    assert!(!body.contains("@"));
    assert!(!body.contains("zoom.us"));
}

#[test]
fn calendar_notification_preferences_default_to_true() {
    let prefs = NotificationPreferences::default();
    assert!(prefs.show_calendar_auto_record_started);
    assert!(prefs.show_calendar_auto_record_skipped);
    assert!(prefs.show_calendar_scheduler_errors);
}

#[test]
fn calendar_notification_preferences_can_be_disabled() {
    let mut prefs = NotificationPreferences::default();
    prefs.show_calendar_auto_record_started = false;
    prefs.show_calendar_auto_record_skipped = false;
    prefs.show_calendar_scheduler_errors = false;

    assert!(!prefs.show_calendar_auto_record_started);
    assert!(!prefs.show_calendar_auto_record_skipped);
    assert!(!prefs.show_calendar_scheduler_errors);
}

#[test]
fn notification_type_match_exhaustive_for_calendar() {
    let types = vec![
        NotificationType::CalendarAutoRecordStarted,
        NotificationType::CalendarAutoRecordSkipped,
        NotificationType::CalendarSchedulerError,
    ];

    for ty in types {
        let result = match ty {
            NotificationType::CalendarAutoRecordStarted => "started",
            NotificationType::CalendarAutoRecordSkipped => "skipped",
            NotificationType::CalendarSchedulerError => "error",
            _ => "other",
        };
        assert_ne!(
            result, "other",
            "Calendar type should not fall through to default"
        );
    }
}

#[test]
fn calendar_auto_record_started_notification_priority_is_high() {
    let notification = Notification::calendar_auto_record_started();
    assert!(matches!(
        notification.priority,
        app_lib::notifications::types::NotificationPriority::High
    ));
}

#[test]
fn calendar_scheduler_error_notification_priority_is_high() {
    let notification = Notification::calendar_scheduler_error("test");
    assert!(matches!(
        notification.priority,
        app_lib::notifications::types::NotificationPriority::High
    ));
}

#[test]
fn calendar_auto_record_skipped_notification_priority_is_normal() {
    let notification = Notification::calendar_auto_record_skipped("test");
    assert!(matches!(
        notification.priority,
        app_lib::notifications::types::NotificationPriority::Normal
    ));
}
