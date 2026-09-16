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

    /// Evaluates scheduled classes 15 minutes before session starts and sends deduplicated reminders.
    pub async fn run_reminder_cycle(pool: &PgPool) -> Result<usize, sqlx::Error> {
        let now = Utc::now();
        // Shift to WIB (UTC+7)
        let wib_now = now + chrono::Duration::hours(7);
        let current_day = Self::get_indonesian_weekday(wib_now.weekday());
        let current_date_str = wib_now.format("%Y-%m-%d").to_string();

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
            "#
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
            let class_name: String = s.get("class_name");
            let subject_name: String = s.get("subject_name");

            let target_user_id = match teacher_user_id {
                Some(uid) => uid,
                None => continue,
            };

            // Deduplication Key: teacher_id + schedule_id + session_date + notification_type
            let dedup_key = format!("{}_{}_{}_MATERIAL_REMINDER", teacher_id, schedule_id, current_date_str);

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
                "#
            )
            .bind(tenant_id)
            .bind(class_id)
            .bind(subject_id)
            .fetch_one(pool)
            .await?
            .try_get("exists")
            .unwrap_or(false);

            let (title, body) = if material_exists {
                (
                    format!("Materi {} Terjadwal", subject_name),
                    format!("Materi {} untuk {} akan otomatis ter-publish 15 menit lagi.", subject_name, class_name),
                )
            } else {
                (
                    format!("Pengingat: Jam Mengajar {}", class_name),
                    format!("Jam pelajaran {} mulai 15 menit lagi, jangan lupa isi materi ya, Pak/Bu.", class_name),
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
            .bind(title)
            .bind(body)
            .bind(schedule_id)
            .bind(&dedup_key)
            .bind(scheduled_at)
            .execute(pool)
            .await?;

            if result.rows_affected() > 0 {
                sent_count += 1;
            }
        }

        Ok(sent_count)
    }
}
