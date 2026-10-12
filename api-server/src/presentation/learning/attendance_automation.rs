use sqlx::{PgPool, Row};
use tracing::info;
use uuid::Uuid;

/// 1. Deteksi kehadiran siswa yang mengikuti pembelajaran online dengan cara membaca materi di hari jadwal pembelajaran tersebut.
/// Jika hari ini bertepatan dengan jadwal pembelajaran kelas siswa untuk mata pelajaran tersebut,
/// status kehadiran siswa otomatis dicatat sebagai 'present' (Hadir Daring).
pub async fn record_online_material_presence(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    material_id: Uuid,
) -> Result<bool, sqlx::Error> {
    // 1. Ambil data siswa berdasarkan user_id
    let student_row = sqlx::query(
        "SELECT id, full_name FROM students WHERE user_id = $1 AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1"
    )
    .bind(user_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;

    let student = match student_row {
        Some(s) => s,
        None => return Ok(false), // Bukan akun siswa
    };
    let student_id: Uuid = student.get("id");

    // 2. Ambil data materi pembelajaran
    let material_row = sqlx::query(
        r#"
        SELECT id, title, class_id, subject_id, session_id 
        FROM learning_materials 
        WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
        "#
    )
    .bind(material_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;

    let material = match material_row {
        Some(m) => m,
        None => return Ok(false),
    };

    let material_title: String = material.get("title");
    let material_class_id: Option<Uuid> = material.get("class_id");
    let material_subject_id: Option<Uuid> = material.get("subject_id");
    let material_session_id: Option<Uuid> = material.get("session_id");

    // 3. Ambil kelas aktif siswa (enrollments)
    let student_class_id: Option<Uuid> = if let Some(cid) = material_class_id {
        Some(cid)
    } else {
        sqlx::query_scalar(
            r#"
            SELECT class_id 
            FROM enrollments 
            WHERE student_id = $1 AND (status IS NULL OR status ILIKE 'active')
            LIMIT 1
            "#
        )
        .bind(student_id)
        .fetch_optional(pool)
        .await?
    };

    let class_id = match student_class_id {
        Some(c) => c,
        None => return Ok(false),
    };

    // 4. Cari atau pastikan sesi pembelajaran hari ini
    let existing_session_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id 
        FROM learning_sessions 
        WHERE tenant_id = $1 
          AND class_id = $2 
          AND session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date
          AND (
              ($3::uuid IS NOT NULL AND subject_id = $3)
              OR ($4::uuid IS NOT NULL AND id = $4)
              OR ($3::uuid IS NULL AND $4::uuid IS NULL)
          )
          AND deleted_at IS NULL
        ORDER BY created_at DESC 
        LIMIT 1
        "#
    )
    .bind(tenant_id)
    .bind(class_id)
    .bind(material_subject_id)
    .bind(material_session_id)
    .fetch_optional(pool)
    .await?;

    let session_id = if let Some(sid) = existing_session_id {
        sid
    } else {
        // Cek apakah hari ini ada jadwal mingguan di class_schedules untuk rombel dan mapel ini
        let schedule_row = sqlx::query(
            r#"
            SELECT cs.id, cs.teacher_id, cs.start_time, cs.end_time, cs.subject_id
            FROM class_schedules cs
            WHERE cs.tenant_id = $1
              AND cs.class_id = $2
              AND (cs.subject_id = $3 OR $3 IS NULL)
              AND cs.day_of_week = CASE EXTRACT(DOW FROM (NOW() AT TIME ZONE 'Asia/Jakarta'))::integer
                  WHEN 0 THEN 'Minggu'
                  WHEN 1 THEN 'Senin'
                  WHEN 2 THEN 'Selasa'
                  WHEN 3 THEN 'Rabu'
                  WHEN 4 THEN 'Kamis'
                  WHEN 5 THEN 'Jumat'
                  WHEN 6 THEN 'Sabtu'
              END
              AND cs.deleted_at IS NULL
            LIMIT 1
            "#
        )
        .bind(tenant_id)
        .bind(class_id)
        .bind(material_subject_id)
        .fetch_optional(pool)
        .await?;

        if let Some(sched) = schedule_row {
            let sched_id: Uuid = sched.get("id");
            let sched_teacher_id: Uuid = sched.get("teacher_id");
            let sched_subject_id: Uuid = sched.get("subject_id");
            let sched_start_time: String = sched.get("start_time");
            let sched_end_time: String = sched.get("end_time");

            let new_session_id = Uuid::new_v4();
            let _ = sqlx::query(
                r#"
                INSERT INTO learning_sessions (
                    id, tenant_id, session_type, schedule_id, class_id, subject_id, teacher_id,
                    session_date, start_time, end_time, status, created_at, updated_at
                )
                VALUES (
                    $1, $2, 'scheduled', $3, $4, $5, $6,
                    (NOW() AT TIME ZONE 'Asia/Jakarta')::date,
                    $7::time, $8::time, 'active', NOW(), NOW()
                )
                ON CONFLICT (id) DO NOTHING
                "#
            )
            .bind(new_session_id)
            .bind(tenant_id)
            .bind(sched_id)
            .bind(class_id)
            .bind(sched_subject_id)
            .bind(sched_teacher_id)
            .bind(sched_start_time)
            .bind(sched_end_time)
            .execute(pool)
            .await;

            new_session_id
        } else if let Some(mat_sess_id) = material_session_id {
            mat_sess_id
        } else {
            // Tidak ada jadwal atau sesi untuk mapel ini hari ini
            return Ok(false);
        }
    };

    // 5. Catat kehadiran siswa sebagai PRESENT (Hadir Daring)
    let note_text = format!("Hadir daring otomatis: Siswa membaca materi '{}'", material_title);
    let result = sqlx::query(
        r#"
        INSERT INTO session_attendances (
            id, tenant_id, session_id, student_id, status, method, notes, checked_in_at, created_at, updated_at
        )
        VALUES (
            gen_random_uuid(), $1, $2, $3, 'present', 'online_material', $4, NOW(), NOW(), NOW()
        )
        ON CONFLICT (session_id, student_id) DO UPDATE
        SET status = 'present',
            method = 'online_material',
            notes = $4,
            checked_in_at = COALESCE(session_attendances.checked_in_at, NOW()),
            updated_at = NOW()
        WHERE session_attendances.status IN ('absent')
        "#
    )
    .bind(tenant_id)
    .bind(session_id)
    .bind(student_id)
    .bind(&note_text)
    .execute(pool)
    .await?;

    info!(
        student_id = %student_id,
        session_id = %session_id,
        material_id = %material_id,
        rows_affected = result.rows_affected(),
        "Auto-attendance recorded for student reading online material on scheduled day"
    );

    Ok(true)
}

/// 2. Proses Auto-Alpa (Aturan 24 Jam):
/// Jika dalam 24 jam siswa tidak memberikan keterangan (izin, sakit) dan tidak membaca materi,
/// seluruh siswa di kelas tersebut yang belum memiliki presensi otomatis dinyatakan 'absent' (Alpa).
pub async fn process_auto_alpa_for_expired_sessions(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO session_attendances (
            id, tenant_id, session_id, student_id, status, method, notes, created_at, updated_at
        )
        SELECT 
            gen_random_uuid(),
            ls.tenant_id,
            ls.id,
            en.student_id,
            'absent',
            'auto_alpa',
            'Alpa otomatis (24 Jam): Tidak membaca materi pembelajaran & tanpa keterangan izin/sakit',
            NOW(),
            NOW()
        FROM learning_sessions ls
        JOIN enrollments en ON en.class_id = ls.class_id AND (en.status IS NULL OR en.status ILIKE 'active')
        LEFT JOIN session_attendances sa ON sa.session_id = ls.id AND sa.student_id = en.student_id
        WHERE ls.tenant_id = $1
          AND ls.deleted_at IS NULL
          AND (
              -- Sesi telah lewat lebih dari 24 jam dari jadwal tanggal/jam pelaksanaan
              ls.session_date < (NOW() AT TIME ZONE 'Asia/Jakarta')::date - INTERVAL '1 day'
              OR (
                  ls.session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date - INTERVAL '1 day'
                  AND (NOW() AT TIME ZONE 'Asia/Jakarta')::time >= COALESCE(ls.end_time, '23:59:59'::time)
              )
          )
          AND sa.id IS NULL
        ON CONFLICT (session_id, student_id) DO NOTHING
        "#
    )
    .bind(tenant_id)
    .execute(pool)
    .await?;

    let affected = result.rows_affected();
    if affected > 0 {
        info!(tenant_id = %tenant_id, count = affected, "Auto-alpa executed for unattended expired sessions (> 24 hours)");
    }

    Ok(affected)
}

/// 3. Pengajuan Keterangan Izin / Sakit oleh Siswa atau Wali Murid melalui Android Mobile App
pub async fn record_student_mobile_excuse(
    pool: &PgPool,
    tenant_id: Uuid,
    user_id: Uuid,
    session_id: Uuid,
    status: &str, // 'sick' or 'excused'
    notes: Option<&str>,
) -> Result<bool, school_core::common::error::ApplicationError> {
    // 1. Validasi status
    let norm_status = match status.to_lowercase().as_str() {
        "sick" | "sakit" => "sick",
        "excused" | "izin" => "excused",
        _ => return Err(school_core::common::error::ApplicationError::Domain(
            school_core::common::error::DomainError::Validation(
                "Status keterangan harus 'sick' (sakit) atau 'excused' (izin)".to_string(),
            ),
        )),
    };

    // 2. Ambil student_id milik user (bisa sebagai siswa atau wali)
    let student_row = sqlx::query(
        r#"
        SELECT s.id as student_id
        FROM students s
        WHERE s.tenant_id = $1 AND (
            s.user_id = $2
            OR s.guardian_id IN (SELECT g.id FROM guardians g WHERE g.user_id = $2)
        )
        LIMIT 1
        "#
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e),
    ))?;

    let student_id: Uuid = match student_row {
        Some(s) => s.get("student_id"),
        None => return Err(school_core::common::error::ApplicationError::Unauthorized(
            school_core::common::error_code::ErrorCode::AuthPermissionDenied,
            "Hanya akun siswa atau wali yang dapat mengajukan surat izin / sakit".to_string(),
        )),
    };

    // 3. Cek apakah sesi valid dan belum melewati batas 24 jam
    let session_row = sqlx::query(
        r#"
        SELECT id, session_date, end_time,
               CASE 
                   WHEN session_date < (NOW() AT TIME ZONE 'Asia/Jakarta')::date - INTERVAL '1 day' THEN true
                   WHEN session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date - INTERVAL '1 day' 
                        AND (NOW() AT TIME ZONE 'Asia/Jakarta')::time >= COALESCE(end_time, '23:59:59'::time) THEN true
                   ELSE false
               END as is_expired
        FROM learning_sessions
        WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL
        "#
    )
    .bind(session_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e),
    ))?;

    let session = match session_row {
        Some(s) => s,
        None => return Err(school_core::common::error::ApplicationError::NotFound(
            school_core::common::error_code::ErrorCode::SessionNotFound,
            "Sesi pembelajaran tidak ditemukan".to_string(),
        )),
    };

    let is_expired: bool = session.get("is_expired");
    if is_expired {
        return Err(school_core::common::error::ApplicationError::Domain(
            school_core::common::error::DomainError::Validation(
                "Batas waktu pengajuan keterangan izin/sakit (maksimal 24 jam setelah sesi) telah berakhir. Status kehadiran telah ditutup menjadi Alpa.".to_string(),
            ),
        ));
    }

    let note_text = notes.unwrap_or("Keterangan diajukan via Android App Mobile").trim();

    // 4. Upsert ke session_attendances
    let _ = sqlx::query(
        r#"
        INSERT INTO session_attendances (
            id, tenant_id, session_id, student_id, status, method, notes, recorded_by, created_at, updated_at
        )
        VALUES (
            gen_random_uuid(), $1, $2, $3, $4, 'mobile_app', $5, $6, NOW(), NOW()
        )
        ON CONFLICT (session_id, student_id) DO UPDATE
        SET status = $4,
            method = 'mobile_app',
            notes = $5,
            recorded_by = $6,
            updated_at = NOW()
        "#
    )
    .bind(tenant_id)
    .bind(session_id)
    .bind(student_id)
    .bind(norm_status)
    .bind(note_text)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| school_core::common::error::ApplicationError::Infrastructure(
        school_core::common::error::InfrastructureError::Database(e),
    ))?;

    Ok(true)
}
