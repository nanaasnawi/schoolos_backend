use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct ScheduledPublishWorker;

impl ScheduledPublishWorker {
    pub async fn run_publish_cycle(pool: &PgPool) -> Result<(), sqlx::Error> {
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
            WHERE m.release_at <= NOW() 
              AND m.notification_dispatched = false 
              AND m.deleted_at IS NULL
              AND m.is_active = true
              AND m.visibility = 'scheduled' 
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
                    JOIN classes en_c ON en_c.id = en.class_id
                    JOIN classes target_c ON target_c.id = $4
                         AND (en_c.id = target_c.id OR (en_c.tingkat IS NOT NULL AND en_c.tingkat != '' AND en_c.tingkat = target_c.tingkat))
                    WHERE en_c.tenant_id = target_c.tenant_id AND (en.status ILIKE 'active')
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
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Material,
                    m_id,
                    crate::infrastructure::fcm::FcmTarget::Classes(sibling_cids),
                );
            } else {
                crate::infrastructure::fcm::trigger_fcm_push_targeted(
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
            WHERE a.release_at <= NOW() 
              AND a.notification_dispatched = false 
              AND a.deleted_at IS NULL
              AND a.is_active = true
              AND a.status = 'scheduled'
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
                    JOIN classes en_c ON en_c.id = en.class_id
                    JOIN classes target_c ON target_c.id = $4
                         AND (en_c.id = target_c.id OR (en_c.tingkat IS NOT NULL AND en_c.tingkat != '' AND en_c.tingkat = target_c.tingkat))
                    WHERE en_c.tenant_id = target_c.tenant_id AND (en.status ILIKE 'active')
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
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Assignment,
                    a_id,
                    crate::infrastructure::fcm::FcmTarget::Classes(sibling_cids),
                );
            } else {
                crate::infrastructure::fcm::trigger_fcm_push_targeted(
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
              AND q.notification_dispatched = false 
              AND q.deleted_at IS NULL
              AND q.is_active = true
              AND q.status = 'scheduled' 
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
                    JOIN classes en_c ON en_c.id = en.class_id
                    JOIN classes target_c ON target_c.id = $4
                         AND (en_c.id = target_c.id OR (en_c.tingkat IS NOT NULL AND en_c.tingkat != '' AND en_c.tingkat = target_c.tingkat))
                    WHERE en_c.tenant_id = target_c.tenant_id AND (en.status ILIKE 'active')
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
                    notif_title,
                    notif_body,
                    crate::infrastructure::fcm::FcmCategory::Quiz,
                    q_id,
                    crate::infrastructure::fcm::FcmTarget::Classes(sibling_cids),
                );
            } else {
                crate::infrastructure::fcm::trigger_fcm_push_targeted(
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

        Ok(())
    }
}

