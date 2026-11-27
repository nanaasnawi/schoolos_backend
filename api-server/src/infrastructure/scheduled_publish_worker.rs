use chrono::{DateTime, Duration, Timelike, Utc};
use sqlx::{PgPool, Row};
use std::collections::{BTreeMap, HashSet};
use tokio::sync::OnceCell;
use uuid::Uuid;

static PRE_START_DISPATCH_SCHEMA_READY: OnceCell<()> = OnceCell::const_new();

async fn ensure_exam_pre_start_dispatch_schema(_pool: &PgPool) {
    // Schema managed exclusively via SQLx migrations (see 20261125020000_add_multitenant_hot_indexes_and_analytics_projection.sql).
    PRE_START_DISPATCH_SCHEMA_READY.get_or_init(|| async {}).await;
}

/// Resets the 5-minute pre-start dispatch guard when an exam schedule is changed.
pub async fn clear_quiz_pre_start_notice(pool: &PgPool, quiz_id: Uuid) {
    ensure_exam_pre_start_dispatch_schema(pool).await;
    let _ = sqlx::query("DELETE FROM quiz_pre_start_notices WHERE quiz_id = $1")
        .bind(quiz_id)
        .execute(pool)
        .await;
}

struct UpcomingExamNoticeItem {
    quiz_id: Uuid,
    tenant_id: Uuid,
    title: String,
    class_id: Option<Uuid>,
    class_name: Option<String>,
    teacher_user_id: Option<Uuid>,
    teacher_name: String,
    start_at: DateTime<Utc>,
    start_wib_label: String,
    start_bucket: String,
}

async fn close_expired_quizzes(pool: &PgPool) {
    match sqlx::query(
        r#"
        UPDATE quizzes
        SET status = 'closed',
            is_active = false,
            updated_at = NOW()
        WHERE deleted_at IS NULL
          AND end_at IS NOT NULL
          AND end_at <= NOW()
          AND (
              is_active = true
              OR LOWER(COALESCE(status, '')) IN ('published', 'active', 'live_exam', 'live', 'scheduled')
          )
        "#,
    )
    .execute(pool)
    .await
    {
        Ok(res) if res.rows_affected() > 0 => {
            tracing::info!(
                component = "scheduled_publish_worker",
                closed_count = res.rows_affected(),
                "Automatically transitioned expired CBT exams to closed"
            );
        }
        Err(e) => {
            tracing::warn!(
                component = "scheduled_publish_worker",
                "Failed to close expired quizzes: {e}"
            );
        }
        _ => {}
    }
}

async fn close_expired_assignments(pool: &PgPool) {
    // Only auto-lock submissions in DB when late submissions are disallowed.
    // Assignments with allow_late_submission=true keep accepting late student attempts while
    // read APIs & UI mark due_at <= NOW() as expired/closed instead of "Aktif Tayang".
    match sqlx::query(
        r#"
        UPDATE assignments
        SET status = 'closed',
            updated_at = NOW()
        WHERE deleted_at IS NULL
          AND due_at IS NOT NULL
          AND due_at <= NOW()
          AND COALESCE(allow_late_submission, false) = false
          AND LOWER(COALESCE(status, '')) IN ('published', 'active')
        "#,
    )
    .execute(pool)
    .await
    {
        Ok(res) if res.rows_affected() > 0 => {
            tracing::info!(
                component = "scheduled_publish_worker",
                closed_count = res.rows_affected(),
                "Automatically transitioned past-due assignments without late submission to closed"
            );
        }
        Err(e) => {
            tracing::warn!(
                component = "scheduled_publish_worker",
                "Failed to close expired assignments: {e}"
            );
        }
        _ => {}
    }
}

async fn dispatch_ready_to_grade_assignment_warnings(pool: &PgPool) {
    let wib_hour = (Utc::now() + Duration::hours(7)).hour();
    if wib_hour < 6 || wib_hour >= 22 {
        return;
    }

    let rows = match sqlx::query(
        r#"
        SELECT
            a.id,
            a.tenant_id,
            a.title,
            COALESCE(c.name, 'Semua Kelas') AS class_name,
            COALESCE(NULLIF(TRIM(t.full_name), ''), ut.full_name, uc.full_name, 'Guru Pengampu') AS teacher_name,
            COALESCE(t.user_id, a.created_by) AS teacher_user_id,
            (
                SELECT COUNT(DISTINCT sub.student_id)::bigint
                FROM assignment_submissions sub
                WHERE sub.assignment_id = a.id
                  AND sub.status IN ('submitted', 'late', 'grading')
            ) AS ungraded_count
        FROM assignments a
        LEFT JOIN classes c ON c.id = a.class_id
        LEFT JOIN teachers t ON t.id = a.teacher_id
        LEFT JOIN users ut ON ut.id = t.user_id
        LEFT JOIN users uc ON uc.id = a.created_by
        WHERE a.deleted_at IS NULL
          AND a.due_at IS NOT NULL
          AND a.due_at <= NOW()
          AND (a.release_at IS NULL OR a.release_at <= NOW())
          AND EXISTS (
              SELECT 1 FROM assignment_submissions sub
              WHERE sub.assignment_id = a.id
                AND sub.status IN ('submitted', 'late', 'grading')
          )
        ORDER BY a.due_at DESC
        LIMIT 30
        "#,
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                component = "scheduled_publish_worker",
                "Ready-to-grade assignment warning query failed: {e}"
            );
            return;
        }
    };

    for row in rows {
        let assignment_id: Uuid = row.get("id");
        let tenant_id: Uuid = row.get("tenant_id");
        let title: String = row.get("title");
        let class_name: String = row.get("class_name");
        let teacher_name: String = row.get("teacher_name");
        let teacher_user_id: Option<Uuid> = row.try_get("teacher_user_id").ok().flatten();
        let ungraded_count: i64 = row.try_get::<i64, _>("ungraded_count").unwrap_or(0);
        if ungraded_count <= 0 {
            continue;
        }

        // 1. Targeted warning to the assigned teacher
        if let Some(t_uid) = teacher_user_id {
            let dedup_key = format!("ASSIGN_READY_GRADE_{}_{}", assignment_id, t_uid);
            let notif_title = format!("⚠️ Tugas Siap Dinilai: {}", title);
            let notif_body = format!(
                "Tenggat tugas kelas {} telah berakhir dan ada {} jawaban siswa menunggu penilaian.",
                class_name, ungraded_count
            );

            if let Ok(res) = sqlx::query(
                r#"
                INSERT INTO notifications (
                    id, tenant_id, user_id, title, body, notification_type, channel,
                    reference_type, reference_id, is_read, dedup_key, scheduled_at, is_urgent, priority, created_at
                )
                VALUES ($1, $2, $3, $4, $5, 'GRADING_WARNING', 'in_app', 'assignment', $6, false, $7, NOW(), true, 'HIGH', NOW())
                ON CONFLICT (tenant_id, dedup_key) WHERE dedup_key IS NOT NULL DO NOTHING
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(tenant_id)
            .bind(t_uid)
            .bind(&notif_title)
            .bind(&notif_body)
            .bind(assignment_id)
            .bind(&dedup_key)
            .execute(pool)
            .await
            {
                if res.rows_affected() > 0 {
                    crate::infrastructure::fcm::trigger_fcm_push_targeted(
                        tenant_id,
                        notif_title,
                        notif_body,
                        crate::infrastructure::fcm::FcmCategory::Assignment,
                        assignment_id,
                        crate::infrastructure::fcm::FcmTarget::User(t_uid),
                    );
                }
            }
        }

        // 2. Targeted supervision warning to Principal(s) in the same tenant
        let principal_uids = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT DISTINCT u.id
            FROM users u
            JOIN user_roles ur ON ur.user_id = u.id
            JOIN roles r ON r.id = ur.role_id
            WHERE u.tenant_id = $1
              AND u.is_active = true
              AND (
                  LOWER(r.name) LIKE '%kepala%'
                  OR LOWER(r.name) LIKE '%principal%'
                  OR LOWER(r.name) LIKE '%headmaster%'
              )
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for p_uid in principal_uids {
            if Some(p_uid) == teacher_user_id {
                continue;
            }
            let dedup_key = format!("PRINCIPAL_GRADE_WARN_{}_{}", assignment_id, p_uid);
            let exec_title = format!("⚠️ Supervisi Tugas Siap Dinilai ({})", class_name);
            let exec_body = format!(
                "{} ({}) oleh {} telah lewat tenggat dengan {} jawaban menunggu koreksi.",
                title, class_name, teacher_name, ungraded_count
            );

            if let Ok(res) = sqlx::query(
                r#"
                INSERT INTO notifications (
                    id, tenant_id, user_id, title, body, notification_type, channel,
                    reference_type, reference_id, is_read, dedup_key, scheduled_at, is_urgent, priority, created_at
                )
                VALUES ($1, $2, $3, $4, $5, 'GRADING_WARNING', 'in_app', 'assignment', $6, false, $7, NOW(), true, 'HIGH', NOW())
                ON CONFLICT (tenant_id, dedup_key) WHERE dedup_key IS NOT NULL DO NOTHING
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(tenant_id)
            .bind(p_uid)
            .bind(&exec_title)
            .bind(&exec_body)
            .bind(assignment_id)
            .bind(&dedup_key)
            .execute(pool)
            .await
            {
                if res.rows_affected() > 0 {
                    crate::infrastructure::fcm::trigger_fcm_push_targeted(
                        tenant_id,
                        exec_title,
                        exec_body,
                        crate::infrastructure::fcm::FcmCategory::Assignment,
                        assignment_id,
                        crate::infrastructure::fcm::FcmTarget::User(p_uid),
                    );
                }
            }
        }
    }
}

async fn dispatch_upcoming_exam_5min_notifications(pool: &PgPool) {
    ensure_exam_pre_start_dispatch_schema(pool).await;

    let rows = match sqlx::query(
        r#"
        SELECT
            q.id,
            q.tenant_id,
            q.title,
            q.class_id,
            c.name AS class_name,
            COALESCE(t.user_id, q.created_by) AS teacher_user_id,
            COALESCE(NULLIF(TRIM(t.full_name), ''), ut.full_name, uc.full_name, 'Guru Pengampu') AS teacher_name,
            COALESCE(q.start_at, q.release_at) AS effective_start
        FROM quizzes q
        LEFT JOIN classes c ON c.id = q.class_id
        LEFT JOIN teachers t ON t.id = q.teacher_id
        LEFT JOIN users ut ON ut.id = t.user_id
        LEFT JOIN users uc ON uc.id = q.created_by
        WHERE q.deleted_at IS NULL
          AND q.is_active = true
          AND COALESCE(q.questions_count, 0) > 0
          AND LOWER(COALESCE(q.status, '')) NOT IN ('closed', 'archived', 'ended')
          AND COALESCE(q.start_at, q.release_at) IS NOT NULL
          AND COALESCE(q.start_at, q.release_at) > NOW()
          AND COALESCE(q.start_at, q.release_at) <= NOW() + INTERVAL '5 minutes 30 seconds'
          AND (q.end_at IS NULL OR q.end_at > COALESCE(q.start_at, q.release_at))
        ORDER BY COALESCE(q.start_at, q.release_at) ASC
        LIMIT 80
        "#,
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                component = "scheduled_publish_worker",
                "Upcoming 5-min exam query failed: {e}"
            );
            return;
        }
    };

    if rows.is_empty() {
        return;
    }

    let mut claimed_batches: BTreeMap<(Uuid, String), Vec<UpcomingExamNoticeItem>> = BTreeMap::new();

    for r in rows {
        let quiz_id: Uuid = r.get("id");
        let tenant_id: Uuid = r.get("tenant_id");
        let start_at: DateTime<Utc> = r.get("effective_start");
        let wib = start_at + Duration::hours(7);
        let bucket_min = (wib.minute() / 5) * 5;
        let start_bucket = format!(
            "{}-{:02}-{:02}",
            wib.format("%Y-%m-%d"),
            wib.hour(),
            bucket_min
        );
        let start_wib_label = format!("{:02}:{:02}", wib.hour(), wib.minute());

        // Atomic per-quiz claim: fires once per scheduled start window
        let claimed = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO quiz_pre_start_notices (quiz_id, tenant_id, start_bucket, notified_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (quiz_id) DO UPDATE
                SET start_bucket = EXCLUDED.start_bucket,
                    notified_at = NOW()
                WHERE quiz_pre_start_notices.start_bucket <> EXCLUDED.start_bucket
            RETURNING quiz_id
            "#,
        )
        .bind(quiz_id)
        .bind(tenant_id)
        .bind(&start_bucket)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();

        if claimed.is_none() {
            continue;
        }

        claimed_batches
            .entry((tenant_id, start_bucket.clone()))
            .or_default()
            .push(UpcomingExamNoticeItem {
                quiz_id,
                tenant_id,
                title: r.get("title"),
                class_id: r.try_get("class_id").ok().flatten(),
                class_name: r.try_get("class_name").ok().flatten(),
                teacher_user_id: r.try_get("teacher_user_id").ok().flatten(),
                teacher_name: r.get("teacher_name"),
                start_at,
                start_wib_label,
                start_bucket,
            });
    }

    for ((tenant_id, start_bucket), items) in claimed_batches {
        if items.is_empty() {
            continue;
        }
        let primary_quiz_id = items[0].quiz_id;
        let earliest_wib = items
            .iter()
            .min_by_key(|it| it.start_at)
            .map(|it| it.start_wib_label.as_str())
            .unwrap_or("segera");

        // 1. Kepsek: TEPAT 1 push notifikasi ringkas untuk seluruh sesi ujian pada window ini
        let principal_uids = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT DISTINCT u.id
            FROM users u
            JOIN user_roles ur ON ur.user_id = u.id
            JOIN roles r ON r.id = ur.role_id
            WHERE u.tenant_id = $1
              AND u.is_active = true
              AND (
                  LOWER(r.name) LIKE '%kepala%'
                  OR LOWER(r.name) LIKE '%principal%'
                  OR LOWER(r.name) LIKE '%headmaster%'
              )
            "#,
        )
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let (exec_title, exec_body) = if items.len() == 1 {
            let ex = &items[0];
            let cls = ex.class_name.as_deref().unwrap_or("Semua Kelas");
            (
                "⏰ Pengawasan Ujian CBT (5 Menit Lagi)".to_string(),
                format!(
                    "{} ({} • {} WIB, Pengampu: {}) segera dimulai. Pantau di Ruang Proktor.",
                    ex.title, cls, ex.start_wib_label, ex.teacher_name
                ),
            )
        } else {
            let mut classes_seen: Vec<String> = Vec::new();
            for it in &items {
                if let Some(ref cn) = it.class_name {
                    if !classes_seen.contains(cn) && classes_seen.len() < 4 {
                        classes_seen.push(cn.clone());
                    }
                }
            }
            let extra = items.len().saturating_sub(classes_seen.len());
            let class_summary = if classes_seen.is_empty() {
                format!("{} sesi ujian", items.len())
            } else if extra > 0 {
                format!("{} (+{} kelas lain)", classes_seen.join(", "), extra)
            } else {
                classes_seen.join(", ")
            };
            (
                format!("⏰ {} Ujian CBT Dimulai 5 Menit Lagi", items.len()),
                format!(
                    "Pukul {} WIB ({}). Siapkan pengawasan di Ruang Proktor.",
                    earliest_wib, class_summary
                ),
            )
        };

        for p_uid in &principal_uids {
            let claimed_exec = sqlx::query_scalar::<_, Uuid>(
                r#"
                INSERT INTO quiz_executive_pre_start_batches (tenant_id, user_id, start_bucket, sent_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (tenant_id, user_id, start_bucket) DO NOTHING
                RETURNING user_id
                "#,
            )
            .bind(tenant_id)
            .bind(p_uid)
            .bind(&start_bucket)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

            if claimed_exec.is_some() {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                    VALUES (gen_random_uuid(), $1, $2, $3, $4, 'QUIZ_REMINDER', 'in_app', 'quiz', $5, FALSE, NOW())
                    "#,
                )
                .bind(tenant_id)
                .bind(p_uid)
                .bind(&exec_title)
                .bind(&exec_body)
                .bind(primary_quiz_id)
                .execute(pool)
                .await;

                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    tenant_id,
                    exec_title.clone(),
                    exec_body.clone(),
                    crate::infrastructure::fcm::FcmCategory::Quiz,
                    primary_quiz_id,
                    crate::infrastructure::fcm::FcmTarget::User(*p_uid),
                );
            }
        }

        // 2. Guru Pengampu: TEPAT 1 push notifikasi ringkas per guru (walau mengampu banyak rombel sekaligus)
        let mut by_teacher: BTreeMap<Uuid, Vec<&UpcomingExamNoticeItem>> = BTreeMap::new();
        for it in &items {
            if let Some(t_uid) = it.teacher_user_id {
                by_teacher.entry(t_uid).or_default().push(it);
            }
        }

        for (t_uid, t_items) in by_teacher {
            let claimed_teacher = sqlx::query_scalar::<_, Uuid>(
                r#"
                INSERT INTO quiz_executive_pre_start_batches (tenant_id, user_id, start_bucket, sent_at)
                VALUES ($1, $2, $3, NOW())
                ON CONFLICT (tenant_id, user_id, start_bucket) DO NOTHING
                RETURNING user_id
                "#,
            )
            .bind(tenant_id)
            .bind(t_uid)
            .bind(&start_bucket)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

            if claimed_teacher.is_none() {
                continue;
            }

            let t_ref_id = t_items[0].quiz_id;
            let t_earliest = t_items
                .iter()
                .min_by_key(|it| it.start_at)
                .map(|it| it.start_wib_label.as_str())
                .unwrap_or(earliest_wib);

            let (t_title, t_body) = if t_items.len() == 1 {
                let single = t_items[0];
                let cls = single.class_name.as_deref().unwrap_or("Semua Kelas");
                (
                    "⏰ Ujian CBT Dimulai 5 Menit Lagi".to_string(),
                    format!(
                        "{} untuk {} siap dimulai pukul {} WIB. Buka Ruang Proktor.",
                        single.title, cls, single.start_wib_label
                    ),
                )
            } else {
                let mut cls_list: Vec<String> = Vec::new();
                for it in &t_items {
                    if let Some(ref c) = it.class_name {
                        if !cls_list.contains(c) && cls_list.len() < 4 {
                            cls_list.push(c.clone());
                        }
                    }
                }
                let extra = t_items.len().saturating_sub(cls_list.len());
                let cls_desc = if cls_list.is_empty() {
                    format!("{} sesi", t_items.len())
                } else if extra > 0 {
                    format!("{} (+{} kelas lain)", cls_list.join(", "), extra)
                } else {
                    cls_list.join(", ")
                };
                (
                    format!("⏰ {} Ujian Anda Dimulai 5 Menit Lagi", t_items.len()),
                    format!(
                        "{} • Pukul {} WIB ({}). Pantau peserta via Ruang Proktor.",
                        t_items[0].title, t_earliest, cls_desc
                    ),
                )
            };

            let _ = sqlx::query(
                r#"
                INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                VALUES (gen_random_uuid(), $1, $2, $3, $4, 'QUIZ_REMINDER', 'in_app', 'quiz', $5, FALSE, NOW())
                "#,
            )
            .bind(tenant_id)
            .bind(t_uid)
            .bind(&t_title)
            .bind(&t_body)
            .bind(t_ref_id)
            .execute(pool)
            .await;

            crate::infrastructure::fcm::trigger_fcm_push_targeted(
                tenant_id,
                t_title,
                t_body,
                crate::infrastructure::fcm::FcmCategory::Quiz,
                t_ref_id,
                crate::infrastructure::fcm::FcmTarget::User(t_uid),
            );
        }

        // 3. Siswa: Push & In-App notification untuk peserta pada tiap rombel ujian
        let mut pushed_student_classes: HashSet<(Uuid, String)> = HashSet::new();
        for ex in &items {
            let cls_label = ex.class_name.as_deref().unwrap_or("Kelas Anda");
            let s_title = "⏰ Ujian CBT Dimulai 5 Menit Lagi!".to_string();
            let s_body = format!(
                "{} ({} • {} WIB). Segera buka portal ujian & bersiap mengerjakan!",
                ex.title, cls_label, ex.start_wib_label
            );

            if let Some(cid) = ex.class_id {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                    SELECT
                        gen_random_uuid(),
                        s.tenant_id,
                        s.user_id,
                        $1,
                        $2,
                        'QUIZ_REMINDER',
                        'in_app',
                        'quiz',
                        $3,
                        FALSE,
                        NOW()
                    FROM students s
                    JOIN enrollments en ON en.student_id = s.id
                    WHERE en.class_id = $4
                      AND (en.status ILIKE 'active' OR en.status IS NULL)
                      AND s.deleted_at IS NULL
                      AND s.user_id IS NOT NULL
                    "#,
                )
                .bind(&s_title)
                .bind(&s_body)
                .bind(ex.quiz_id)
                .bind(cid)
                .execute(pool)
                .await;

                if pushed_student_classes.insert((cid, ex.start_bucket.clone())) {
                    crate::infrastructure::fcm::trigger_fcm_push_targeted(
                        ex.tenant_id,
                        s_title,
                        s_body,
                        crate::infrastructure::fcm::FcmCategory::Quiz,
                        ex.quiz_id,
                        crate::infrastructure::fcm::FcmTarget::Classes(vec![cid]),
                    );
                }
            } else {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                    SELECT
                        gen_random_uuid(),
                        s.tenant_id,
                        s.user_id,
                        $1,
                        $2,
                        'QUIZ_REMINDER',
                        'in_app',
                        'quiz',
                        $3,
                        FALSE,
                        NOW()
                    FROM students s
                    WHERE s.tenant_id = $4
                      AND s.deleted_at IS NULL
                      AND s.is_active = true
                      AND s.user_id IS NOT NULL
                    "#,
                )
                .bind(&s_title)
                .bind(&s_body)
                .bind(ex.quiz_id)
                .bind(ex.tenant_id)
                .execute(pool)
                .await;

                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    ex.tenant_id,
                    s_title,
                    s_body,
                    crate::infrastructure::fcm::FcmCategory::Quiz,
                    ex.quiz_id,
                    crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()]),
                );
            }
        }
    }
}

pub struct ScheduledPublishWorker;

impl ScheduledPublishWorker {
    pub async fn run_publish_cycle(pool: &PgPool) -> Result<(), sqlx::Error> {
        let mut lock_conn = pool.acquire().await?;
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(8842001)")
            .fetch_one(&mut *lock_conn)
            .await
            .unwrap_or(false);
        if !locked {
            return Ok(());
        }

        let cycle_res = Self::run_publish_cycle_inner(pool).await;
        let _ = sqlx::query("SELECT pg_advisory_unlock(8842001)")
            .execute(&mut *lock_conn)
            .await;
        cycle_res
    }

    async fn run_publish_cycle_inner(pool: &PgPool) -> Result<(), sqlx::Error> {
        close_expired_quizzes(pool).await;
        close_expired_assignments(pool).await;
        dispatch_ready_to_grade_assignment_warnings(pool).await;
        dispatch_upcoming_exam_5min_notifications(pool).await;
        // ── 1. Publish & Notify Scheduled Learning Materials ──
        let pending_materials = sqlx::query(
            r#"
            SELECT 
                m.id, m.tenant_id, m.title, m.class_id,
                COALESCE(ut.full_name, uc.full_name, 'Guru Pengampu') as teacher_name,
                c.name as class_name
            FROM learning_materials m
            LEFT JOIN classes c ON c.id = m.class_id
            LEFT JOIN teachers t ON t.id = m.teacher_id
            LEFT JOIN users ut ON ut.id = t.user_id
            LEFT JOIN users uc ON uc.id = m.created_by
            WHERE m.release_at IS NOT NULL
              AND m.release_at <= NOW() 
              AND m.notification_dispatched = false 
              AND m.deleted_at IS NULL
              AND m.is_active = true
              AND (m.visibility = 'scheduled' OR m.visibility = 'draft' OR m.visibility = 'published' OR m.visibility = 'active') 
            LIMIT 25
            "#
        )
        .fetch_all(pool)
        .await?;

        for row in pending_materials {
            let m_id: Uuid = row.get("id");
            let tenant_id: Uuid = row.get("tenant_id");
            let title: String = row.get("title");
            let class_id: Option<Uuid> = row.try_get("class_id").ok().flatten();
            let teacher_name: String = row.get("teacher_name");
            let class_name: Option<String> = row.try_get("class_name").ok().flatten();

            // Mark as published & dispatched
            let _ = sqlx::query(
                r#"
                UPDATE learning_materials 
                SET visibility = 'published', notification_dispatched = true, is_active = true 
                WHERE id = $1
                "#
            )
            .bind(m_id)
            .execute(pool)
            .await;

            // Generate/resolve thumbnail ONCE when scheduled material transitions to published.
            let _ = crate::infrastructure::thumbnail_resolver::ensure_material_thumbnail_on_publish(
                pool.clone(),
                m_id,
            )
            .await;

            let notif_title = format!("📚 Materi Baru: {}", title);
            let notif_body = if let Some(ref c_name) = class_name {
                format!("{} telah menerbitkan modul pembelajaran baru untuk kelas {}. Pelajari sekarang!", teacher_name, c_name)
            } else {
                format!("{} telah menerbitkan modul pembelajaran baru: {}. Buka aplikasi untuk membaca!", teacher_name, title)
            };

            // In-app notifications
            if let Some(cid) = class_id {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                    SELECT 
                        gen_random_uuid(),
                        s.tenant_id,
                        s.user_id,
                        $1,
                        $2,
                        'LEARNING_MATERIAL',
                        'in_app',
                        'material',
                        $3,
                        FALSE,
                        NOW()
                    FROM students s
                    JOIN enrollments en ON en.student_id = s.id
                    WHERE en.class_id = $4 AND (en.status ILIKE 'active' OR en.status IS NULL) AND s.deleted_at IS NULL
                    "#
                )
                .bind(&notif_title)
                .bind(&notif_body)
                .bind(m_id)
                .bind(cid)
                .execute(pool)
                .await;

                let sibling_cids = vec![cid];

                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    tenant_id,
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Material,
                    m_id,
                    crate::infrastructure::fcm::FcmTarget::Classes(sibling_cids),
                );
            } else {
                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    tenant_id,
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Material,
                    m_id,
                    crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()]),
                );
            }

            tracing::info!(
                component = "scheduled_publish_worker",
                material_id = %m_id,
                tenant_id = %tenant_id,
                "Activated and dispatched scheduled learning material"
            );
        }

        // ── 2. Publish & Notify Scheduled Assignments ──
        let pending_assignments = sqlx::query(
            r#"
            SELECT 
                a.id, a.tenant_id, a.title, a.class_id, a.subject_id,
                COALESCE(ut.full_name, uc.full_name, 'Guru Pengampu') as teacher_name,
                c.name as class_name
            FROM assignments a
            LEFT JOIN classes c ON c.id = a.class_id
            LEFT JOIN teachers t ON t.id = a.teacher_id
            LEFT JOIN users ut ON ut.id = t.user_id
            LEFT JOIN users uc ON uc.id = a.created_by
            WHERE a.release_at IS NOT NULL
              AND a.release_at <= NOW() 
              AND a.notification_dispatched = false 
              AND a.deleted_at IS NULL
              AND a.is_active = true
              AND (a.status = 'scheduled' OR a.status = 'draft' OR a.status = 'published' OR a.status = 'active')
            LIMIT 25
            "#
        )
        .fetch_all(pool)
        .await?;

        for row in pending_assignments {
            let a_id: Uuid = row.get("id");
            let tenant_id: Uuid = row.get("tenant_id");
            let title: String = row.get("title");
            let class_id: Option<Uuid> = row.try_get("class_id").ok().flatten();
            let teacher_name: String = row.get("teacher_name");
            let class_name: Option<String> = row.try_get("class_name").ok().flatten();
            let subject_id: Option<Uuid> = row.try_get("subject_id").ok().flatten();

            // Mark as published & dispatched
            let _ = sqlx::query(
                r#"
                UPDATE assignments 
                SET status = 'published', notification_dispatched = true, is_active = true 
                WHERE id = $1
                "#
            )
            .bind(a_id)
            .execute(pool)
            .await;

            // Generate 1x cover thumbnail via FLUX AI when scheduled assignment goes live.
            let _ = crate::infrastructure::thumbnail_resolver::ensure_assignment_thumbnail_on_publish(
                pool.clone(),
                a_id,
            )
            .await;

            // Auto-archive any older active assignments for this class & subject
            if let (Some(cid), Some(sid)) = (class_id, subject_id) {
                let _ = sqlx::query(
                    r#"
                    UPDATE assignments
                    SET status = 'archived', is_active = false, updated_at = NOW()
                    WHERE tenant_id = $1 AND class_id = $2 AND subject_id = $3 AND id != $4 AND is_active = true
                    "#
                )
                .bind(tenant_id)
                .bind(cid)
                .bind(sid)
                .bind(a_id)
                .execute(pool)
                .await;
            }

            let notif_title = format!("📝 Tugas Baru: {}", title);
            let notif_body = if let Some(ref c_name) = class_name {
                format!("{} telah menerbitkan tugas baru untuk kelas {}. Selesaikan sekarang!", teacher_name, c_name)
            } else {
                format!("{} telah menerbitkan tugas baru: {}. Buka aplikasi untuk mengerjakan!", teacher_name, title)
            };

            if let Some(cid) = class_id {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                    SELECT 
                        gen_random_uuid(),
                        s.tenant_id,
                        s.user_id,
                        $1,
                        $2,
                        'ASSIGNMENT',
                        'in_app',
                        'assignment',
                        $3,
                        FALSE,
                        NOW()
                    FROM students s
                    JOIN enrollments en ON en.student_id = s.id
                    WHERE en.class_id = $4 AND (en.status ILIKE 'active' OR en.status IS NULL) AND s.deleted_at IS NULL
                    "#
                )
                .bind(&notif_title)
                .bind(&notif_body)
                .bind(a_id)
                .bind(cid)
                .execute(pool)
                .await;

                let sibling_cids = vec![cid];

                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    tenant_id,
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Assignment,
                    a_id,
                    crate::infrastructure::fcm::FcmTarget::Classes(sibling_cids),
                );
            } else {
                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    tenant_id,
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Assignment,
                    a_id,
                    crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()]),
                );
            }

            tracing::info!(
                component = "scheduled_publish_worker",
                assignment_id = %a_id,
                tenant_id = %tenant_id,
                "Activated and dispatched scheduled assignment"
            );
        }

        // ── 3. Publish & Notify Scheduled Quizzes / CBT ──
        let pending_quizzes = sqlx::query(
            r#"
            SELECT 
                q.id, q.tenant_id, q.title, q.class_id,
                COALESCE(ut.full_name, uc.full_name, 'Guru Pengampu') as teacher_name,
                c.name as class_name
            FROM quizzes q
            LEFT JOIN classes c ON c.id = q.class_id
            LEFT JOIN teachers t ON t.id = q.teacher_id
            LEFT JOIN users ut ON ut.id = t.user_id
            LEFT JOIN users uc ON uc.id = q.created_by
            WHERE q.release_at <= NOW() 
              AND (q.end_at IS NULL OR q.end_at > NOW())
              AND q.notification_dispatched = false 
              AND q.deleted_at IS NULL
              AND q.is_active = true
              AND (q.status = 'scheduled' OR q.status = 'draft') 
            LIMIT 25
            "#
        )
        .fetch_all(pool)
        .await?;

        for row in pending_quizzes {
            let q_id: Uuid = row.get("id");
            let tenant_id: Uuid = row.get("tenant_id");
            let title: String = row.get("title");
            let class_id: Option<Uuid> = row.try_get("class_id").ok().flatten();
            let teacher_name: String = row.get("teacher_name");
            let class_name: Option<String> = row.try_get("class_name").ok().flatten();

            // Mark as published & dispatched
            let _ = sqlx::query(
                r#"
                UPDATE quizzes 
                SET status = 'published', notification_dispatched = true, is_active = true 
                WHERE id = $1
                "#
            )
            .bind(q_id)
            .execute(pool)
            .await;

            let notif_title = format!("🎯 Kuis & Asesmen Baru: {}", title);
            let notif_body = if let Some(ref c_name) = class_name {
                format!("{} telah membuka kuis/asesmen baru untuk kelas {}. Kerjakan sekarang!", teacher_name, c_name)
            } else {
                format!("{} telah membuka kuis baru: {}. Buka aplikasi untuk memulai ujian!", teacher_name, title)
            };

            if let Some(cid) = class_id {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO notifications (id, tenant_id, user_id, title, body, notification_type, channel, reference_type, reference_id, is_read, created_at)
                    SELECT 
                        gen_random_uuid(),
                        s.tenant_id,
                        s.user_id,
                        $1,
                        $2,
                        'QUIZ',
                        'in_app',
                        'quiz',
                        $3,
                        FALSE,
                        NOW()
                    FROM students s
                    JOIN enrollments en ON en.student_id = s.id
                    WHERE en.class_id = $4 AND (en.status ILIKE 'active' OR en.status IS NULL) AND s.deleted_at IS NULL
                    "#
                )
                .bind(&notif_title)
                .bind(&notif_body)
                .bind(q_id)
                .bind(cid)
                .execute(pool)
                .await;

                let sibling_cids = vec![cid];

                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    tenant_id,
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Quiz,
                    q_id,
                    crate::infrastructure::fcm::FcmTarget::Classes(sibling_cids),
                );
            } else {
                crate::infrastructure::fcm::trigger_fcm_push_targeted(
                    tenant_id,
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Quiz,
                    q_id,
                    crate::infrastructure::fcm::FcmTarget::Roles(vec!["student".to_string()]),
                );
            }

            tracing::info!(
                component = "scheduled_publish_worker",
                quiz_id = %q_id,
                tenant_id = %tenant_id,
                "Activated and dispatched scheduled quiz"
            );
        }

        // Backfill already-published items that still have pending thumbnails (strictly capped + 1x claimed).
        crate::infrastructure::thumbnail_resolver::backfill_published_thumbnails_batch(pool).await;

        Ok(())
    }
}

