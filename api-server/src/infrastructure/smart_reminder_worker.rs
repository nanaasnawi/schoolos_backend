use chrono::{DateTime, Datelike, Timelike, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct SmartReminderWorker;

impl SmartReminderWorker {
    /// Computes scheduled_at based on Quiet Hours (21:00 - 06:00 WIB, UTC+7).
    /// If urgent, sends immediately. If created during quiet hours, schedules for 06:00 AM WIB.
    pub fn compute_delivery_time(now: DateTime<Utc>, is_urgent: bool) -> DateTime<Utc> {
        if is_urgent {
            return now;
        }

        // WIB is UTC+7
        let wib_hour = (now.hour() + 7) % 24;

        // Quiet hours: 21:00 to 05:59 WIB
        if wib_hour >= 21 || wib_hour < 6 {
            let hours_until_6am = if wib_hour >= 21 {
                (24 - wib_hour) + 6
            } else {
                6 - wib_hour
            };

            // Set to next 06:00 WIB
            now + chrono::Duration::hours(hours_until_6am as i64)
        } else {
            now
        }
    }

    /// Converts Datelike weekday into Indonesian day name matching class_schedules.day_of_week
    pub fn get_indonesian_weekday(weekday: chrono::Weekday) -> &'static str {
        match weekday {
            chrono::Weekday::Mon => "Senin",
            chrono::Weekday::Tue => "Selasa",
            chrono::Weekday::Wed => "Rabu",
            chrono::Weekday::Thu => "Kamis",
            chrono::Weekday::Fri => "Jumat",
            chrono::Weekday::Sat => "Sabtu",
            chrono::Weekday::Sun => "Minggu",
        }
    }

    /// Parses time string into NaiveTime. Supports formats HH:MM and HH:MM:SS.
    pub fn parse_time_str(time_str: &str) -> Option<chrono::NaiveTime> {
        let trimmed = time_str.trim();
        chrono::NaiveTime::parse_from_str(trimmed, "%H:%M")
            .ok()
            .or_else(|| chrono::NaiveTime::parse_from_str(trimmed, "%H:%M:%S").ok())
            .or_else(|| {
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() >= 2 {
                    let h: u32 = parts[0].parse().ok()?;
                    let m: u32 = parts[1].parse().ok()?;
                    let s: u32 = if parts.len() >= 3 {
                        parts[2].parse().ok().unwrap_or(0)
                    } else {
                        0
                    };
                    chrono::NaiveTime::from_hms_opt(h, m, s)
                } else {
                    None
                }
            })
    }

    /// Evaluates if current WIB time is within the 15-minute reminder window before class start.
    /// Centered at 15 minutes before (e.g. 13 to 16 minutes remaining).
    pub fn is_within_reminder_window(
        scheduled_time: chrono::NaiveTime,
        current_wib_time: chrono::NaiveTime,
    ) -> bool {
        let sched_secs = scheduled_time.num_seconds_from_midnight() as i64;
        let curr_secs = current_wib_time.num_seconds_from_midnight() as i64;
        let diff_secs = sched_secs - curr_secs;
        let minutes_until_start = diff_secs / 60;

        // Window: exactly 13 to 16 minutes before session start time
        minutes_until_start >= 13 && minutes_until_start <= 16
    }

    /// Evaluates scheduled classes 15 minutes before session starts and sends deduplicated reminders.
    /// Strictly restricted to school operational hours (06:00 - 21:00 WIB) and targeted to the assigned teacher.
    pub async fn run_reminder_cycle(pool: &PgPool) -> Result<usize, sqlx::Error> {
        let now = Utc::now();
        // Shift to WIB (UTC+7)
        let wib_now = now + chrono::Duration::hours(7);
        let wib_hour = wib_now.hour();

        // 1. Strict Quiet Hours Guard: Never send teaching reminder alarms between 21:00 and 06:00 WIB!
        if wib_hour >= 21 || wib_hour < 6 {
            return Ok(0);
        }

        let current_day = Self::get_indonesian_weekday(wib_now.weekday());
        let current_date_str = wib_now.format("%Y-%m-%d").to_string();
        let current_wib_time = wib_now.time();

        let schedules = sqlx::query(
            r#"
            SELECT 
                cs.id as schedule_id,
                cs.tenant_id,
                cs.class_id,
                cs.subject_id,
                cs.teacher_id,
                cs.start_time,
                t.user_id as teacher_user_id,
                t.full_name as teacher_name,
                c.name as class_name,
                sub.name as subject_name
            FROM class_schedules cs
            JOIN teachers t ON t.id = cs.teacher_id
            JOIN classes c ON c.id = cs.class_id
            JOIN subjects sub ON sub.id = cs.subject_id
            WHERE cs.day_of_week ILIKE $1 
              AND cs.deleted_at IS NULL
            "#,
        )
        .bind(current_day)
        .fetch_all(pool)
        .await?;

        let mut sent_count = 0;

        for s in schedules {
            let schedule_id: Uuid = s.get("schedule_id");
            let tenant_id: Uuid = s.get("tenant_id");
            let class_id: Uuid = s.get("class_id");
            let subject_id: Uuid = s.get("subject_id");
            let teacher_id: Uuid = s.get("teacher_id");
            let teacher_user_id: Option<Uuid> = s.get("teacher_user_id");
            let start_time_raw: String = s.get("start_time");
            let class_name: String = s.get("class_name");
            let subject_name: String = s.get("subject_name");

            // 2. Strict Start Time Filter: Only alert 15 minutes before the teaching session starts!
            let Some(st) = Self::parse_time_str(&start_time_raw) else {
                continue;
            };
            if !Self::is_within_reminder_window(st, current_wib_time) {
                continue;
            }

            // 3. Strict Teacher Filter: Only alert the teacher assigned to this rombel!
            let target_user_id = match teacher_user_id {
                Some(uid) => uid,
                None => continue,
            };

            // Deduplication Key: teacher_id + schedule_id + session_date + notification_type
            let dedup_key = format!(
                "{}_{}_{}_MATERIAL_REMINDER",
                teacher_id, schedule_id, current_date_str
            );

            // Check if material is already available for this subject, class & teacher
            let material_exists = sqlx::query(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM learning_materials 
                    WHERE tenant_id = $1 
                      AND class_id = $2 
                      AND subject_id = $3 
                      AND deleted_at IS NULL
                ) as exists
                "#,
            )
            .bind(tenant_id)
            .bind(class_id)
            .bind(subject_id)
            .fetch_one(pool)
            .await?
            .try_get("exists")
            .unwrap_or(false);

            let time_formatted = format!("{:02}:{:02} WIB", st.hour(), st.minute());

            let (title, body) = if material_exists {
                (
                    format!("⏰ 15 Menit Lagi: Jadwal Mengajar {}", class_name),
                    format!(
                        "Jam pelajaran {} ({}) dimulai pukul {}. Modul materi Anda sudah siap untuk kelas ini.",
                        class_name, subject_name, time_formatted
                    ),
                )
            } else {
                (
                    format!("⏰ 15 Menit Lagi: Jadwal Mengajar {}", class_name),
                    format!(
                        "Jam pelajaran {} ({}) dimulai pukul {}. Jangan lupa siapkan dan isi materi pembelajaran, tugas, atau kuis ya, Pak/Bu.",
                        class_name, subject_name, time_formatted
                    ),
                )
            };

            let notif_id = Uuid::new_v4();
            let scheduled_at = Self::compute_delivery_time(now, false);

            let result = sqlx::query(
                r#"
                INSERT INTO notifications (
                    id, tenant_id, user_id, title, body, notification_type, channel,
                    reference_type, reference_id, is_read, dedup_key, scheduled_at, is_urgent, priority, created_at
                )
                VALUES ($1, $2, $3, $4, $5, 'SMART_REMINDER', 'in_app', 'material', $6, false, $7, $8, false, 'NORMAL', NOW())
                ON CONFLICT (tenant_id, dedup_key) WHERE dedup_key IS NOT NULL DO NOTHING
                "#
            )
            .bind(notif_id)
            .bind(tenant_id)
            .bind(target_user_id)
            .bind(title.clone())
            .bind(body.clone())
            .bind(schedule_id)
            .bind(&dedup_key)
            .bind(scheduled_at)
            .execute(pool)
            .await?;

            if result.rows_affected() > 0 {
                sent_count += 1;
                // FCM push notification strictly TARGETED to the teacher's user ID (never broadcast to FcmTarget::All)
                let fcm_title = title.clone();
                let fcm_body = body.clone();
                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    fcm_title,
                    fcm_body,
                    crate::infrastructure::fcm::FcmCategory::Reminder,
                    schedule_id,
                    crate::infrastructure::fcm::FcmTarget::User(target_user_id),
                );
            }
        }

        Ok(sent_count)
    }
}
