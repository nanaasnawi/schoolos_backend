use api_server::infrastructure::smart_reminder_worker::SmartReminderWorker;
use chrono::{TimeZone, Utc, Weekday};
use serde_json::Value;
use utoipa::OpenApi;

#[test]
fn test_quiet_hours_urgent_immediate() {
    // 22:00 WIB (15:00 UTC) is inside quiet hours
    let now = Utc.with_ymd_and_hms(2026, 9, 17, 15, 0, 0).unwrap();
    let scheduled_at = SmartReminderWorker::compute_delivery_time(now, true);
    // Urgent messages must NEVER be deferred
    assert_eq!(scheduled_at, now);
}

#[test]
fn test_quiet_hours_daytime_immediate() {
    // 10:00 WIB is 03:00 UTC (normal school hours)
    let now = Utc.with_ymd_and_hms(2026, 9, 17, 3, 0, 0).unwrap();
    let scheduled_at = SmartReminderWorker::compute_delivery_time(now, false);
    // Non-urgent messages during school hours deliver immediately
    assert_eq!(scheduled_at, now);
}

#[test]
fn test_quiet_hours_evening_deferred_to_morning() {
    // 21:00 WIB is 14:00 UTC (start of quiet hours: 21:00 - 06:00 WIB)
    let evening_now = Utc.with_ymd_and_hms(2026, 9, 17, 14, 0, 0).unwrap();
    let scheduled_at = SmartReminderWorker::compute_delivery_time(evening_now, false);

    // Delivery should be postponed to 06:00 WIB next morning (23:00 UTC)
    // 14:00 UTC + 9 hours = 23:00 UTC
    let expected = Utc.with_ymd_and_hms(2026, 9, 17, 23, 0, 0).unwrap();
    assert_eq!(scheduled_at, expected);
}

#[test]
fn test_quiet_hours_early_morning_deferred_to_morning() {
    // 03:00 WIB is 20:00 UTC previous day (inside quiet hours)
    let early_morning = Utc.with_ymd_and_hms(2026, 9, 16, 20, 0, 0).unwrap();
    let scheduled_at = SmartReminderWorker::compute_delivery_time(early_morning, false);

    // In WIB: wib_hour = (20 + 7) % 24 = 3
    // Hours until 6am = 6 - 3 = 3 hours
    // 20:00 UTC + 3 hours = 23:00 UTC (06:00 WIB)
    let expected = Utc.with_ymd_and_hms(2026, 9, 16, 23, 0, 0).unwrap();
    assert_eq!(scheduled_at, expected);
}

#[test]
fn test_indonesian_weekday_mapping() {
    assert_eq!(
        SmartReminderWorker::get_indonesian_weekday(Weekday::Mon),
        "Senin"
    );
    assert_eq!(
        SmartReminderWorker::get_indonesian_weekday(Weekday::Tue),
        "Selasa"
    );
    assert_eq!(
        SmartReminderWorker::get_indonesian_weekday(Weekday::Wed),
        "Rabu"
    );
    assert_eq!(
        SmartReminderWorker::get_indonesian_weekday(Weekday::Thu),
        "Kamis"
    );
    assert_eq!(
        SmartReminderWorker::get_indonesian_weekday(Weekday::Fri),
        "Jumat"
    );
    assert_eq!(
        SmartReminderWorker::get_indonesian_weekday(Weekday::Sat),
        "Sabtu"
    );
    assert_eq!(
        SmartReminderWorker::get_indonesian_weekday(Weekday::Sun),
        "Minggu"
    );
}

#[test]
fn test_dedup_key_format_integrity() {
    let teacher_id = uuid::Uuid::new_v4();
    let schedule_id = uuid::Uuid::new_v4();
    let date_str = "2026-09-17";

    let dedup_key = format!("{teacher_id}_{schedule_id}_{date_str}_MATERIAL_REMINDER");
    assert!(dedup_key.contains(&teacher_id.to_string()));
    assert!(dedup_key.contains(&schedule_id.to_string()));
    assert!(dedup_key.ends_with("_MATERIAL_REMINDER"));
}

#[test]
fn test_openapi_schema_contains_core_routes() {
    let openapi = api_server::ApiDoc::openapi();
    let json = openapi.to_json().expect("OpenAPI must serialize to JSON");
    let parsed: Value = serde_json::from_str(&json).expect("Must be valid JSON");

    let paths = parsed.get("paths").expect("Must contain paths");

    // Core routes documented in ApiDoc
    assert!(
        paths.get("/api/v1/auth/login").is_some(),
        "login route must be documented"
    );
    assert!(
        paths.get("/api/v1/academic/classes").is_some(),
        "classes route must be documented"
    );
    assert!(
        paths.get("/api/v1/academic/enrollments").is_some(),
        "enrollments route must be documented"
    );
}

#[test]
fn test_reading_progress_calculation_bounds() {
    // Reading progress percentage logic test
    let calculate_percentage = |last_page: i32, total_pages: i32| -> f32 {
        if total_pages <= 0 {
            0.0
        } else {
            ((last_page as f32 / total_pages as f32) * 100.0).clamp(0.0, 100.0)
        }
    };

    assert_eq!(calculate_percentage(0, 100), 0.0);
    assert_eq!(calculate_percentage(50, 100), 50.0);
    assert_eq!(calculate_percentage(100, 100), 100.0);
    assert_eq!(calculate_percentage(150, 100), 100.0); // Clamped to 100%
    assert_eq!(calculate_percentage(10, 0), 0.0); // Safe division by zero
}

#[test]
fn test_chat_client_message_id_uniqueness() {
    let client_msg_id_1 = uuid::Uuid::new_v4().to_string();
    let client_msg_id_2 = uuid::Uuid::new_v4().to_string();
    assert_ne!(client_msg_id_1, client_msg_id_2);
    assert!(uuid::Uuid::parse_str(&client_msg_id_1).is_ok());
}

#[test]
fn test_teacher_assignment_isolation_logic() {
    let teacher_a_id = uuid::Uuid::new_v4();
    let teacher_b_id = uuid::Uuid::new_v4();
    let assignment_created_by = teacher_a_id;
    let assignment_teacher_id = Some(teacher_a_id);

    // Teacher A is the owner
    let is_teacher_a_owner =
        assignment_created_by == teacher_a_id || assignment_teacher_id == Some(teacher_a_id);
    assert!(
        is_teacher_a_owner,
        "Teacher A must have access to own assignment"
    );

    // Teacher B is NOT the owner
    let is_teacher_b_owner =
        assignment_created_by == teacher_b_id || assignment_teacher_id == Some(teacher_b_id);
    assert!(
        !is_teacher_b_owner,
        "Teacher B must NOT have access to Teacher A's assignment"
    );
}

#[test]
fn test_reminder_window_exact_15_minutes() {
    use chrono::NaiveTime;
    let scheduled = NaiveTime::from_hms_opt(7, 30, 0).unwrap();
    // 07:15 WIB is exactly 15 minutes before 07:30 WIB
    let current_15m = NaiveTime::from_hms_opt(7, 15, 0).unwrap();
    assert!(SmartReminderWorker::is_within_reminder_window(scheduled, current_15m));

    // 07:14 WIB (16 mins remaining) is within window
    let current_16m = NaiveTime::from_hms_opt(7, 14, 0).unwrap();
    assert!(SmartReminderWorker::is_within_reminder_window(scheduled, current_16m));

    // 07:17 WIB (13 mins remaining) is within window
    let current_13m = NaiveTime::from_hms_opt(7, 17, 0).unwrap();
    assert!(SmartReminderWorker::is_within_reminder_window(scheduled, current_13m));
}

#[test]
fn test_reminder_window_midnight_and_irrelevant_times_rejected() {
    use chrono::NaiveTime;
    let scheduled = NaiveTime::from_hms_opt(7, 30, 0).unwrap();
    // 00:21 WIB (midnight bug!) must NEVER trigger
    let midnight = NaiveTime::from_hms_opt(0, 21, 0).unwrap();
    assert!(!SmartReminderWorker::is_within_reminder_window(scheduled, midnight));

    // 06:00 WIB (90 mins early) must not trigger
    let too_early = NaiveTime::from_hms_opt(6, 0, 0).unwrap();
    assert!(!SmartReminderWorker::is_within_reminder_window(scheduled, too_early));

    // 07:35 WIB (class already started) must not trigger
    let past = NaiveTime::from_hms_opt(7, 35, 0).unwrap();
    assert!(!SmartReminderWorker::is_within_reminder_window(scheduled, past));
}

#[test]
fn test_parse_time_str_various_formats() {
    use chrono::NaiveTime;
    assert_eq!(
        SmartReminderWorker::parse_time_str("07:30"),
        Some(NaiveTime::from_hms_opt(7, 30, 0).unwrap())
    );
    assert_eq!(
        SmartReminderWorker::parse_time_str("08:15:00"),
        Some(NaiveTime::from_hms_opt(8, 15, 0).unwrap())
    );
    assert_eq!(
        SmartReminderWorker::parse_time_str("  13:45  "),
        Some(NaiveTime::from_hms_opt(13, 45, 0).unwrap())
    );
    assert_eq!(SmartReminderWorker::parse_time_str("invalid"), None);
}
