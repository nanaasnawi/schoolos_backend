use chrono::Datelike;
use sqlx::{PgPool, Row};
use tokio::sync::OnceCell;
use uuid::Uuid;

static GRADE_SYNC_SCHEMA_READY: OnceCell<()> = OnceCell::const_new();

/// Schema is maintained exclusively by migrations; kept as a zero-DDL compatibility shim.
pub async fn ensure_grade_sync_schema(_pool: &PgPool) {
    GRADE_SYNC_SCHEMA_READY.get_or_init(|| async {}).await;
}

/// Normalizes an explicit or title-inferred assessment category into canonical values:
/// `'FORMATIVE' | 'SUMMATIVE_M1' | 'SUMMATIVE_M2' | 'SUMMATIVE_M3' | 'ATS' | 'SAS'`.
pub fn normalize_assessment_category(raw: Option<&str>, title_hint: Option<&str>) -> String {
    if let Some(val) = raw.map(str::trim).filter(|s| !s.is_empty()) {
        let up = val.to_uppercase();
        if up == "SAS" || up == "PAS" || up == "UAS" || up == "SUMMATIVE_SEMESTER" {
            return "SAS".to_string();
        }
        if up == "ATS" || up == "UTS" || up == "PTS" || up == "EXAM_MID" {
            return "ATS".to_string();
        }
        if up == "SUMMATIVE_M1" || up == "M1" || up == "MODUL_1" {
            return "SUMMATIVE_M1".to_string();
        }
        if up == "SUMMATIVE_M2" || up == "M2" || up == "MODUL_2" {
            return "SUMMATIVE_M2".to_string();
        }
        if up == "SUMMATIVE_M3" || up == "M3" || up == "MODUL_3" {
            return "SUMMATIVE_M3".to_string();
        }
        if up == "FORMATIVE" || up == "FORMATIF" || up == "DAILY" || up == "TUGAS" {
            return "FORMATIVE".to_string();
        }
    }

    if let Some(title) = title_hint {
        let up = title.to_uppercase();
        let has_sas = up.contains("SAS")
            || up.contains("PAS")
            || up.contains("SUMATIF AKHIR")
            || up.contains("PENILAIAN AKHIR")
            || up.contains("UJIAN AKHIR");
        if has_sas {
            return "SAS".to_string();
        }
        let has_ats = up.contains("UTS")
            || up.contains("ATS")
            || up.contains("PTS")
            || up.contains("TENGAH SEMESTER");
        if has_ats {
            return "ATS".to_string();
        }
        if up.contains("SUMATIF MODUL 3") || up.contains("SUMATIF M3") || up.contains("BAB 3") {
            return "SUMMATIVE_M3".to_string();
        }
        if up.contains("SUMATIF MODUL 2") || up.contains("SUMATIF M2") || up.contains("BAB 2") {
            return "SUMMATIVE_M2".to_string();
        }
        if up.contains("SUMATIF MODUL 1") || up.contains("SUMATIF M1") || up.contains("SUMATIF") {
            return "SUMMATIVE_M1".to_string();
        }
    }

    "FORMATIVE".to_string()
}

pub fn qualitative_level_from_score(score: f64) -> &'static str {
    if score >= 88.0 {
        "SB"
    } else if score >= 75.0 {
        "BSH"
    } else if score >= 60.0 {
        "MB"
    } else {
        "BM"
    }
}

/// Keeps teacher's custom manual notes while refreshing the `[Auto-Formatif: ...]` summary line.
pub fn merge_auto_formative_notes(existing: Option<&str>, auto_summary: Option<String>) -> Option<String> {
    let custom_lines: Vec<&str> = existing
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("[Auto-Formatif"))
        .collect();

    match (auto_summary, custom_lines.is_empty()) {
        (Some(auto), true) => Some(auto),
        (Some(auto), false) => Some(format!("{}\n{}", auto, custom_lines.join("\n"))),
        (None, true) => None,
        (None, false) => Some(custom_lines.join("\n")),
    }
}

#[derive(Debug, Clone)]
pub struct ActiveAcademicContext {
    pub academic_year_id: Option<Uuid>,
    pub academic_year_name: String,
    pub semester_code: String, // "ODD" | "EVEN"
}

pub async fn resolve_active_academic_context(pool: &PgPool, tenant_id: Uuid) -> ActiveAcademicContext {
    let row = sqlx::query(
        r#"
        SELECT id, name, COALESCE(current_semester, 1) AS current_semester
        FROM academic_years
        WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some(r) = row {
        let ay_id: Option<Uuid> = r.try_get("id").ok();
        let ay_name: String = r.try_get("name").unwrap_or_else(|_| "2026/2027".to_string());
        let sem_num: i16 = r.try_get("current_semester").unwrap_or(1);
        let sem_code = if sem_num == 2 { "EVEN" } else { "ODD" }.to_string();
        ActiveAcademicContext {
            academic_year_id: ay_id,
            academic_year_name: ay_name,
            semester_code: sem_code,
        }
    } else {
        let now = chrono::Utc::now();
        let sem_code = if (1..=6).contains(&now.month()) { "EVEN" } else { "ODD" }.to_string();
        ActiveAcademicContext {
            academic_year_id: None,
            academic_year_name: "2026/2027".to_string(),
            semester_code: sem_code,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_gradebook_snapshot(
    pool: &PgPool,
    tenant_id: Uuid,
    academic_year: &str,
    semester_code: &str,
    class_id: Uuid,
    student_id: Uuid,
    subject_code: &str,
    subject_name: &str,
    avg_summative_tp: Option<f64>,
    ats_score: Option<f64>,
    sas_score: Option<f64>,
    final_score: Option<f64>,
    kktp: f64,
    highest_desc: &str,
    lowest_desc: &str,
) {
    let Some(final_val) = final_score else {
        return;
    };

    ensure_grade_sync_schema(pool).await;

    let clean_sub_code: String = subject_code.trim().chars().take(50).collect();
    let clean_sub_name: String = subject_name.trim().chars().take(100).collect();
    let clean_sem = if semester_code.eq_ignore_ascii_case("EVEN") {
        "EVEN"
    } else {
        "ODD"
    };
    let status_kktp = if final_val >= kktp { "TUNTAS" } else { "PERLU_REMEDIAL" };
    let avg_tp_val = avg_summative_tp.unwrap_or(0.0);

    let res = sqlx::query(
        r#"
        INSERT INTO gradebook_snapshots (
            tenant_id, academic_year, semester, class_id, student_id,
            subject_code, subject_name,
            avg_summative_tp, ats_score, sas_score, final_score, status_kktp,
            highest_achievement_desc, lowest_improvement_desc,
            is_stale, calculated_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6, $7,
            $8::float8::numeric, $9::float8::numeric, $10::float8::numeric, $11::float8::numeric, $12,
            $13, $14,
            false, NOW(), NOW()
        )
        ON CONFLICT (tenant_id, academic_year, semester, student_id, subject_code) DO UPDATE
        SET class_id = EXCLUDED.class_id,
            subject_name = EXCLUDED.subject_name,
            avg_summative_tp = EXCLUDED.avg_summative_tp,
            ats_score = COALESCE(EXCLUDED.ats_score, gradebook_snapshots.ats_score),
            sas_score = COALESCE(EXCLUDED.sas_score, gradebook_snapshots.sas_score),
            final_score = EXCLUDED.final_score,
            status_kktp = EXCLUDED.status_kktp,
            highest_achievement_desc = EXCLUDED.highest_achievement_desc,
            lowest_improvement_desc = EXCLUDED.lowest_improvement_desc,
            is_stale = false,
            calculated_at = NOW(),
            updated_at = NOW()
        "#,
    )
    .bind(tenant_id)
    .bind(academic_year)
    .bind(clean_sem)
    .bind(class_id)
    .bind(student_id)
    .bind(&clean_sub_code)
    .bind(&clean_sub_name)
    .bind(avg_tp_val)
    .bind(ats_score)
    .bind(sas_score)
    .bind(final_val)
    .bind(status_kktp)
    .bind(highest_desc)
    .bind(lowest_desc)
    .execute(pool)
    .await;

    if let Err(e) = res {
        tracing::warn!("upsert_gradebook_snapshot failed for student {student_id}: {e}");
    }
}

async fn resolve_canonical_student_and_class(
    pool: &PgPool,
    tenant_id: Uuid,
    raw_student_or_user_id: Uuid,
    hint_class_id: Option<Uuid>,
) -> Option<(Uuid, Uuid)> {
    let row = sqlx::query(
        r#"
        SELECT s.id AS student_id,
               COALESCE(
                   $3::uuid,
                   (SELECT en.class_id FROM enrollments en
                    WHERE en.student_id = s.id
                      AND (en.status ILIKE 'active' OR en.status IS NULL)
                    ORDER BY en.enrolled_at DESC NULLS LAST LIMIT 1)
               ) AS resolved_class_id
        FROM students s
        WHERE s.tenant_id = $1
          AND s.deleted_at IS NULL
          AND (s.id = $2 OR s.user_id = $2)
        LIMIT 1
        "#,
    )
    .bind(tenant_id)
    .bind(raw_student_or_user_id)
    .bind(hint_class_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;

    let student_id: Uuid = row.get("student_id");
    let class_id: Option<Uuid> = row.try_get("resolved_class_id").ok().flatten();
    class_id.map(|cid| (student_id, cid))
}

async fn resolve_canonical_subject(
    pool: &PgPool,
    tenant_id: Uuid,
    hint_subject_id: Option<Uuid>,
    description_hint: Option<&str>,
) -> Option<(Uuid, String, String)> {
    if let Some(sid) = hint_subject_id {
        if let Ok(Some(row)) = sqlx::query(
            "SELECT id, COALESCE(NULLIF(TRIM(code), ''), id::text) AS code, name FROM subjects WHERE id = $1 AND tenant_id = $2 LIMIT 1",
        )
        .bind(sid)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        {
            return Some((row.get("id"), row.get("code"), row.get("name")));
        }
    }

    if let Some(desc) = description_hint {
        let first_part = desc.split('•').next().map(str::trim).unwrap_or("");
        if !first_part.is_empty() && !first_part.eq_ignore_ascii_case("Umum") {
            if let Ok(Some(row)) = sqlx::query(
                r#"
                SELECT id, COALESCE(NULLIF(TRIM(code), ''), id::text) AS code, name
                FROM subjects
                WHERE tenant_id = $1
                  AND (name ILIKE $2 OR code ILIKE $2 OR name ILIKE '%' || $2 || '%')
                ORDER BY (LOWER(name) = LOWER($2)) DESC
                LIMIT 1
                "#,
            )
            .bind(tenant_id)
            .bind(first_part)
            .fetch_optional(pool)
            .await
            {
                return Some((row.get("id"), row.get("code"), row.get("name")));
            }
        }
    }

    None
}

#[allow(clippy::too_many_arguments)]
async fn upsert_pedagogical_source_result(
    pool: &PgPool,
    tenant_id: Uuid,
    academic_ctx: &ActiveAcademicContext,
    class_id: Uuid,
    subject_code: &str,
    subject_name: &str,
    source_type: &str,
    source_id: Uuid,
    title: &str,
    assessment_category: &str,
    method: &str,
    student_id: Uuid,
    score: f64,
    feedback_notes: Option<&str>,
) {
    let taxonomy_type = match assessment_category {
        "SUMMATIVE_M1" | "SUMMATIVE_M2" | "SUMMATIVE_M3" => "SUMMATIVE_MATERIAL",
        "ATS" | "SAS" => "SUMMATIVE_SEMESTER",
        _ => "FORMATIVE",
    };

    let assess_title: String = format!("[Otomatis] {}", title.trim())
        .chars()
        .take(255)
        .collect();
    let sub_code: String = subject_code.trim().chars().take(50).collect();
    let sub_name: String = subject_name.trim().chars().take(100).collect();

    let assess_row = sqlx::query(
        r#"
        INSERT INTO pedagogical_assessments (
            tenant_id, academic_year, semester, class_id,
            subject_code, subject_name, title, taxonomy_type,
            assessment_method, passing_threshold, status,
            source_type, source_id
        ) VALUES (
            $1, $2, $3, $4,
            $5, $6, $7, $8,
            $9, 75.0, 'COMPLETED',
            $10, $11
        )
        ON CONFLICT (tenant_id, source_type, source_id) WHERE source_id IS NOT NULL AND deleted_at IS NULL DO UPDATE
        SET title = EXCLUDED.title,
            taxonomy_type = EXCLUDED.taxonomy_type,
            subject_code = EXCLUDED.subject_code,
            subject_name = EXCLUDED.subject_name,
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(tenant_id)
    .bind(&academic_ctx.academic_year_name)
    .bind(&academic_ctx.semester_code)
    .bind(class_id)
    .bind(&sub_code)
    .bind(&sub_name)
    .bind(&assess_title)
    .bind(taxonomy_type)
    .bind(method)
    .bind(source_type)
    .bind(source_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let Some(arow) = assess_row else {
        return;
    };
    let assessment_id: Uuid = arow.get("id");
    let qual_level = qualitative_level_from_score(score);

    let _ = sqlx::query(
        r#"
        INSERT INTO assessment_student_results (
            tenant_id, assessment_id, student_id, raw_score,
            qualitative_level, feedback_notes
        ) VALUES ($1, $2, $3, $4::float8::numeric, $5, $6)
        ON CONFLICT (assessment_id, student_id) DO UPDATE
        SET raw_score = EXCLUDED.raw_score,
            qualitative_level = EXCLUDED.qualitative_level,
            feedback_notes = COALESCE(EXCLUDED.feedback_notes, assessment_student_results.feedback_notes),
            updated_at = NOW()
        "#,
    )
    .bind(tenant_id)
    .bind(assessment_id)
    .bind(student_id)
    .bind(score)
    .bind(qual_level)
    .bind(feedback_notes)
    .execute(pool)
    .await;
}

/// Rebuilds a student's `gradebooks`, `gradebook_entries`, and `gradebook_snapshots` row
/// for `(student_id, class_id, subject_id)` by merging synced graded Assignments + Quizzes/CBT
/// with any existing manually-entered slots in `gradebook_entries`.
pub async fn recompute_student_subject_gradebook(
    pool: &PgPool,
    tenant_id: Uuid,
    student_id: Uuid,
    class_id: Uuid,
    subject_id: Uuid,
    subject_code: &str,
    subject_name: &str,
) -> Result<(), sqlx::Error> {
    ensure_grade_sync_schema(pool).await;
    let academic_ctx = resolve_active_academic_context(pool, tenant_id).await;

    let student_user_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM students WHERE id = $1 AND tenant_id = $2 LIMIT 1",
    )
    .bind(student_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    // 1. Collect all graded assignments for this student & subject
    let asg_rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (a.id)
            a.id AS item_id,
            a.title,
            a.description,
            a.subject_id,
            COALESCE(NULLIF(TRIM(a.assessment_category), ''), 'FORMATIVE') AS assessment_category,
            sub.score,
            GREATEST(COALESCE(a.max_score, 100), 1) AS max_score,
            sub.feedback,
            sub.graded_at,
            sub.updated_at
        FROM assignment_submissions sub
        JOIN assignments a ON a.id = sub.assignment_id
        WHERE a.tenant_id = $1
          AND a.deleted_at IS NULL
          AND (sub.student_id = $2 OR ($3::uuid IS NOT NULL AND sub.student_id = $3))
          AND (sub.status ILIKE 'graded' OR sub.score IS NOT NULL)
          AND (
              a.subject_id = $4
              OR (a.subject_id IS NULL AND split_part(COALESCE(a.description, ''), '•', 1) ILIKE '%' || $5 || '%')
          )
        ORDER BY a.id, sub.graded_at DESC NULLS LAST, sub.updated_at DESC NULLS LAST
        "#,
    )
    .bind(tenant_id)
    .bind(student_id)
    .bind(student_user_id)
    .bind(subject_id)
    .bind(subject_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // 2. Collect all graded quizzes/CBT for this student & subject
    let quiz_rows = sqlx::query(
        r#"
        SELECT DISTINCT ON (q.id)
            q.id AS item_id,
            q.title,
            q.description,
            q.subject_id,
            COALESCE(NULLIF(TRIM(q.assessment_category), ''), 'FORMATIVE') AS assessment_category,
            qa.score,
            qa.total_points,
            q.max_score,
            qa.updated_at
        FROM quiz_attempts qa
        JOIN quizzes q ON q.id = qa.quiz_id
        WHERE qa.tenant_id = $1
          AND q.tenant_id = $1
          AND q.deleted_at IS NULL
          AND (qa.student_id = $2 OR ($3::uuid IS NOT NULL AND qa.student_id = $3))
          AND qa.status ILIKE 'graded'
          AND qa.score IS NOT NULL
          AND (
              q.subject_id = $4
              OR (q.subject_id IS NULL AND split_part(COALESCE(q.description, ''), '•', 1) ILIKE '%' || $5 || '%')
          )
        ORDER BY q.id, qa.updated_at DESC NULLS LAST
        "#,
    )
    .bind(tenant_id)
    .bind(student_id)
    .bind(student_user_id)
    .bind(subject_id)
    .bind(subject_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Group synced scores into buckets
    let mut formatif_scores: Vec<f64> = Vec::new();
    let mut formatif_titles: Vec<String> = Vec::new();
    let mut m1_scores: Vec<f64> = Vec::new();
    let mut m2_scores: Vec<f64> = Vec::new();
    let mut m3_scores: Vec<f64> = Vec::new();
    let mut ats_scores: Vec<f64> = Vec::new();
    let mut sas_scores: Vec<f64> = Vec::new();

    for r in &asg_rows {
        let raw_score: Option<i32> = r.try_get("score").ok().flatten();
        let Some(sc) = raw_score else { continue };
        let max_sc: i32 = r.try_get("max_score").unwrap_or(100).max(1);
        let title: String = r.get("title");
        let raw_cat: String = r.get("assessment_category");
        let cat = normalize_assessment_category(Some(&raw_cat), Some(&title));
        let pct = ((sc as f64) * 100.0 / (max_sc as f64)).clamp(0.0, 100.0);
        match cat.as_str() {
            "SUMMATIVE_M1" => m1_scores.push(pct),
            "SUMMATIVE_M2" => m2_scores.push(pct),
            "SUMMATIVE_M3" => m3_scores.push(pct),
            "ATS" => ats_scores.push(pct),
            "SAS" => sas_scores.push(pct),
            _ => {
                formatif_scores.push(pct);
                formatif_titles.push(title);
            }
        }
    }

    for r in &quiz_rows {
        let raw_score: Option<i32> = r.try_get("score").ok().flatten();
        let Some(sc) = raw_score else { continue };
        let total_pts: i32 = r.try_get::<Option<i32>, _>("total_points").ok().flatten().unwrap_or(0);
        let max_sc: i32 = r.try_get::<Option<i32>, _>("max_score").ok().flatten().unwrap_or(0);
        let eff_max = if total_pts > 0 {
            total_pts
        } else if max_sc > 0 {
            max_sc
        } else {
            100
        };
        let title: String = r.get("title");
        let raw_cat: String = r.get("assessment_category");
        let cat = normalize_assessment_category(Some(&raw_cat), Some(&title));
        let pct = ((sc as f64) * 100.0 / (eff_max as f64)).clamp(0.0, 100.0);
        match cat.as_str() {
            "SUMMATIVE_M1" => m1_scores.push(pct),
            "SUMMATIVE_M2" => m2_scores.push(pct),
            "SUMMATIVE_M3" => m3_scores.push(pct),
            "ATS" => ats_scores.push(pct),
            "SAS" => sas_scores.push(pct),
            _ => {
                formatif_scores.push(pct);
                formatif_titles.push(title);
            }
        }
    }

    let avg_of = |vals: &[f64]| -> Option<f64> {
        if vals.is_empty() {
            None
        } else {
            Some((vals.iter().sum::<f64>() / vals.len() as f64 * 10.0).round() / 10.0)
        }
    };

    // 3. Preserve manual entries from gradebook_entries / gradebooks for slots without synced scores
    let existing_gb = sqlx::query(
        r#"
        SELECT ats_score::float8 AS ats_score,
               sas_score::float8 AS sas_score,
               COALESCE(kktp::float8, 75.0) AS kktp,
               highest_competency_desc,
               lowest_competency_desc,
               formatif_notes
        FROM gradebooks
        WHERE student_id = $1 AND class_id = $2 AND subject_id = $3
        LIMIT 1
        "#,
    )
    .bind(student_id)
    .bind(class_id)
    .bind(subject_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let existing_entries = sqlx::query(
        r#"
        SELECT component_name, source_type, raw_score::float8 AS raw_score
        FROM gradebook_entries
        WHERE student_id = $1 AND class_id = $2 AND subject_id = $3
        "#,
    )
    .bind(student_id)
    .bind(class_id)
    .bind(subject_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut manual_m1: Option<f64> = None;
    let mut manual_m2: Option<f64> = None;
    let mut manual_m3: Option<f64> = None;
    let mut manual_m4: Option<f64> = None;
    let mut manual_ats: Option<f64> = existing_gb.as_ref().and_then(|r| r.try_get("ats_score").ok().flatten());
    let mut manual_sas: Option<f64> = existing_gb.as_ref().and_then(|r| r.try_get("sas_score").ok().flatten());
    let existing_notes: Option<String> = existing_gb.as_ref().and_then(|r| r.try_get("formatif_notes").ok().flatten());
    let kktp: f64 = existing_gb
        .as_ref()
        .and_then(|r| r.try_get("kktp").ok())
        .unwrap_or(75.0);

    for er in &existing_entries {
        let cname: String = er.get("component_name");
        let raw: Option<f64> = er.try_get("raw_score").ok().flatten();
        let Some(v) = raw.filter(|x| *x > 0.0) else { continue };
        if cname == "Sumatif M1" {
            manual_m1 = Some(v);
        } else if cname == "Sumatif M2" {
            manual_m2 = Some(v);
        } else if cname == "Sumatif M3" {
            manual_m3 = Some(v);
        } else if cname == "Sumatif M4" {
            manual_m4 = Some(v);
        } else if cname.starts_with("ATS") && manual_ats.is_none() {
            manual_ats = Some(v);
        } else if cname.starts_with("SAS") && manual_sas.is_none() {
            manual_sas = Some(v);
        }
    }

    let m1 = avg_of(&m1_scores).or(manual_m1);
    let m2 = avg_of(&m2_scores).or(manual_m2);
    let m3 = avg_of(&m3_scores).or(manual_m3);
    let m4 = manual_m4;
    let ats_val = avg_of(&ats_scores).or(manual_ats);
    let sas_val = avg_of(&sas_scores).or(manual_sas);
    let formatif_avg = avg_of(&formatif_scores);

    let mut sumatif_list: Vec<(String, f64)> = Vec::new();
    if let Some(v) = m1 { sumatif_list.push(("Sumatif M1".to_string(), v)); }
    if let Some(v) = m2 { sumatif_list.push(("Sumatif M2".to_string(), v)); }
    if let Some(v) = m3 { sumatif_list.push(("Sumatif M3".to_string(), v)); }
    if let Some(v) = m4 { sumatif_list.push(("Sumatif M4".to_string(), v)); }

    let avg_sumatif = if !sumatif_list.is_empty() {
        Some((sumatif_list.iter().map(|(_, s)| *s).sum::<f64>() / sumatif_list.len() as f64 * 100.0).round() / 100.0)
    } else {
        None
    };

    let mut components: Vec<(String, String, f64, f64, f64)> = Vec::new();
    let final_score_opt = match (avg_sumatif, ats_val, sas_val) {
        (Some(avg_m), Some(ats), Some(sas)) => {
            let w_m = 60.0;
            let w_ats = 20.0;
            let w_sas = 20.0;
            let per_m_w = w_m / sumatif_list.len() as f64;
            for (name, sc) in &sumatif_list {
                components.push((name.clone(), "summative_module".to_string(), *sc, *sc * (per_m_w / 100.0), per_m_w));
            }
            components.push(("ATS (Tengah Semester)".to_string(), "exam_mid".to_string(), ats, ats * (w_ats / 100.0), w_ats));
            components.push(("SAS (Akhir Semester)".to_string(), "exam_final".to_string(), sas, sas * (w_sas / 100.0), w_sas));
            Some((avg_m * w_m + ats * w_ats + sas * w_sas) / 100.0)
        }
        (Some(avg_m), Some(ats), None) => {
            let w_m = 60.0;
            let w_ats = 40.0;
            let per_m_w = w_m / sumatif_list.len() as f64;
            for (name, sc) in &sumatif_list {
                components.push((name.clone(), "summative_module".to_string(), *sc, *sc * (per_m_w / 100.0), per_m_w));
            }
            components.push(("ATS (Tengah Semester)".to_string(), "exam_mid".to_string(), ats, ats * (w_ats / 100.0), w_ats));
            Some((avg_m * w_m + ats * w_ats) / 100.0)
        }
        (Some(avg_m), None, Some(sas)) => {
            let w_m = 60.0;
            let w_sas = 40.0;
            let per_m_w = w_m / sumatif_list.len() as f64;
            for (name, sc) in &sumatif_list {
                components.push((name.clone(), "summative_module".to_string(), *sc, *sc * (per_m_w / 100.0), per_m_w));
            }
            components.push(("SAS (Akhir Semester)".to_string(), "exam_final".to_string(), sas, sas * (w_sas / 100.0), w_sas));
            Some((avg_m * w_m + sas * w_sas) / 100.0)
        }
        (Some(avg_m), None, None) => {
            let per_m_w = 100.0 / sumatif_list.len() as f64;
            for (name, sc) in &sumatif_list {
                components.push((name.clone(), "summative_module".to_string(), *sc, *sc * (per_m_w / 100.0), per_m_w));
            }
            Some(avg_m)
        }
        (None, Some(ats), Some(sas)) => {
            components.push(("ATS (Tengah Semester)".to_string(), "exam_mid".to_string(), ats, ats * 0.5, 50.0));
            components.push(("SAS (Akhir Semester)".to_string(), "exam_final".to_string(), sas, sas * 0.5, 50.0));
            Some((ats + sas) / 2.0)
        }
        (None, Some(ats), None) => {
            components.push(("ATS (Tengah Semester)".to_string(), "exam_mid".to_string(), ats, ats, 100.0));
            Some(ats)
        }
        (None, None, Some(sas)) => {
            components.push(("SAS (Akhir Semester)".to_string(), "exam_final".to_string(), sas, sas, 100.0));
            Some(sas)
        }
        (None, None, None) => None,
    };

    if let Some(favg) = formatif_avg {
        components.push(("Formatif Harian (Tugas & Kuis)".to_string(), "formative_qualitative".to_string(), favg, 0.0, 0.0));
    }

    let auto_summary = formatif_avg.map(|favg| {
        let count = formatif_scores.len();
        let latest_title = formatif_titles.last().map(String::as_str).unwrap_or("Tugas Harian");
        format!("[Auto-Formatif: Rata-rata {:.0} ({} tugas/kuis dinilai, terakhir: {})]", favg.round(), count, latest_title)
    });
    let merged_notes = merge_auto_formative_notes(existing_notes.as_deref(), auto_summary);

    let final_score = final_score_opt.map(|na| na.round());
    let letter_grade = match final_score {
        Some(s) if s >= 88.0 => "A",
        Some(s) if s >= 75.0 => "B",
        Some(s) if s >= 65.0 => "C",
        Some(_) => "D",
        None => "-",
    };
    let passed = final_score.map(|s| s >= kktp).unwrap_or(false);
    let status = match final_score {
        Some(_) if passed => "Tuntas KKTP",
        Some(_) => "Perlu Remedial",
        None => "Belum Diinput",
    };

    let (highest_desc, lowest_desc) = if !sumatif_list.is_empty() {
        let max_elem = sumatif_list.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        let min_elem = sumatif_list.iter().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        let h = if max_elem.1 >= 85.0 {
            format!("Menunjukkan penguasaan yang sangat baik dalam mencapai capaian pembelajaran {} ({}).", subject_name, max_elem.0)
        } else if max_elem.1 >= 75.0 {
            format!("Menunjukkan penguasaan yang baik dalam mencapai capaian pembelajaran {} ({}).", subject_name, max_elem.0)
        } else {
            format!("Mencapai penguasaan cukup dalam {} ({}).", subject_name, max_elem.0)
        };
        let l = if min_elem.1 < kktp {
            format!("Perlu bimbingan dan pendampingan intensif dalam meningkatkan kompetensi {} ({}).", subject_name, min_elem.0)
        } else if min_elem.1 < 80.0 {
            format!("Perlu penguatan dan latihan berkala untuk memperdalam pemahaman {} ({}).", subject_name, min_elem.0)
        } else {
            format!("Menunjukkan konsistensi penguasaan kompetensi yang merata pada mata pelajaran {}.", subject_name)
        };
        (h, l)
    } else if let Some(sc) = final_score {
        let h = if sc >= 85.0 {
            format!("Menunjukkan penguasaan yang sangat baik pada evaluasi mata pelajaran {}.", subject_name)
        } else if sc >= 75.0 {
            format!("Menunjukkan penguasaan yang baik pada evaluasi mata pelajaran {}.", subject_name)
        } else {
            format!("Mencapai pemahaman dasar mata pelajaran {}.", subject_name)
        };
        let l = if sc < kktp {
            format!("Perlu pendampingan belajar intensif pada kompetensi {}.", subject_name)
        } else {
            "Perlu penguatan latihan secara berkala.".to_string()
        };
        (h, l)
    } else {
        (
            format!("Aktif mengikuti penugasan formatif harian {}.", subject_name),
            "Menunggu penilaian sumatif / evaluasi semester.".to_string(),
        )
    };

    let gb_id = Uuid::now_v7();
    let final_str = final_score.map(|v| format!("{:.2}", v));
    let avg_s_str = avg_sumatif.map(|v| format!("{:.2}", v));
    let ats_str = ats_val.map(|v| format!("{:.2}", v));
    let sas_str = sas_val.map(|v| format!("{:.2}", v));
    let kktp_str = format!("{:.2}", kktp);

    sqlx::query(
        r#"
        INSERT INTO gradebooks (
            id, tenant_id, student_id, class_id, subject_id, academic_year_id,
            final_score, letter_grade, passed, status,
            avg_sumatif, ats_score, sas_score, kktp,
            highest_competency_desc, lowest_competency_desc, formatif_notes,
            created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6,
            $7::text::numeric, $8, $9, $10,
            $11::text::numeric, $12::text::numeric, $13::text::numeric, $14::text::numeric,
            $15, $16, $17,
            NOW(), NOW()
        )
        ON CONFLICT (student_id, class_id, subject_id) DO UPDATE
        SET final_score = COALESCE(EXCLUDED.final_score, gradebooks.final_score),
            letter_grade = COALESCE(EXCLUDED.letter_grade, gradebooks.letter_grade),
            passed = EXCLUDED.passed,
            status = EXCLUDED.status,
            avg_sumatif = COALESCE(EXCLUDED.avg_sumatif, gradebooks.avg_sumatif),
            ats_score = COALESCE(EXCLUDED.ats_score, gradebooks.ats_score),
            sas_score = COALESCE(EXCLUDED.sas_score, gradebooks.sas_score),
            kktp = COALESCE(EXCLUDED.kktp, gradebooks.kktp),
            highest_competency_desc = COALESCE(EXCLUDED.highest_competency_desc, gradebooks.highest_competency_desc),
            lowest_competency_desc = COALESCE(EXCLUDED.lowest_competency_desc, gradebooks.lowest_competency_desc),
            formatif_notes = COALESCE(EXCLUDED.formatif_notes, gradebooks.formatif_notes),
            updated_at = NOW()
        "#,
    )
    .bind(gb_id)
    .bind(tenant_id)
    .bind(student_id)
    .bind(class_id)
    .bind(subject_id)
    .bind(academic_ctx.academic_year_id)
    .bind(&final_str)
    .bind(letter_grade)
    .bind(passed)
    .bind(status)
    .bind(&avg_s_str)
    .bind(&ats_str)
    .bind(&sas_str)
    .bind(&kktp_str)
    .bind(&highest_desc)
    .bind(&lowest_desc)
    .bind(&merged_notes)
    .execute(pool)
    .await?;

    // Refresh component entries cleanly
    let _ = sqlx::query(
        "DELETE FROM gradebook_entries WHERE student_id = $1 AND class_id = $2 AND subject_id = $3",
    )
    .bind(student_id)
    .bind(class_id)
    .bind(subject_id)
    .execute(pool)
    .await;

    for (comp_name, src_type, raw, weighted, weight_pct) in components {
        let _ = sqlx::query(
            r#"
            INSERT INTO gradebook_entries (
                id, tenant_id, student_id, class_id, subject_id,
                component_name, source_type, raw_score, max_raw_score,
                weighted_score, weight_percentage, calculated_at, created_at
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8::text::numeric, 100.0,
                $9::text::numeric, $10::text::numeric, NOW(), NOW()
            )
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(tenant_id)
        .bind(student_id)
        .bind(class_id)
        .bind(subject_id)
        .bind(&comp_name)
        .bind(&src_type)
        .bind(format!("{:.2}", raw))
        .bind(format!("{:.2}", weighted))
        .bind(format!("{:.2}", weight_pct))
        .execute(pool)
        .await;
    }

    // Also keep materialized snapshot synced for e-Rapor (/dashboard/grading/final-grades & /dashboard/reports/cards)
    upsert_gradebook_snapshot(
        pool,
        tenant_id,
        &academic_ctx.academic_year_name,
        &academic_ctx.semester_code,
        class_id,
        student_id,
        subject_code,
        subject_name,
        avg_sumatif,
        ats_val,
        sas_val,
        final_score,
        kktp,
        &highest_desc,
        &lowest_desc,
    )
    .await;

    Ok(())
}

/// Syncs a graded assignment submission to `pedagogical_assessments`, `gradebooks`, and `gradebook_snapshots`.
pub async fn sync_assignment_submission_to_gradebook(
    pool: &PgPool,
    tenant_id: Uuid,
    submission_id: Uuid,
) {
    ensure_grade_sync_schema(pool).await;

    let row = sqlx::query(
        r#"
        SELECT sub.student_id,
               sub.score,
               sub.feedback,
               a.id AS assignment_id,
               a.title,
               a.description,
               a.class_id,
               a.subject_id,
               GREATEST(COALESCE(a.max_score, 100), 1) AS max_score,
               a.assignment_type,
               COALESCE(NULLIF(TRIM(a.assessment_category), ''), 'FORMATIVE') AS assessment_category
        FROM assignment_submissions sub
        JOIN assignments a ON a.id = sub.assignment_id
        WHERE sub.id = $1 AND a.tenant_id = $2
        LIMIT 1
        "#,
    )
    .bind(submission_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let Some(r) = row else { return };
    let raw_score_opt: Option<i32> = r.try_get("score").ok().flatten();
    let Some(raw_score) = raw_score_opt else { return };

    let raw_student_id: Uuid = r.get("student_id");
    let hint_class_id: Option<Uuid> = r.try_get("class_id").ok().flatten();
    let hint_subject_id: Option<Uuid> = r.try_get("subject_id").ok().flatten();
    let desc: Option<String> = r.try_get("description").ok().flatten();
    let title: String = r.get("title");
    let assignment_id: Uuid = r.get("assignment_id");
    let feedback: Option<String> = r.try_get("feedback").ok().flatten();
    let max_score: i32 = r.try_get("max_score").unwrap_or(100).max(1);
    let asg_type: String = r.try_get("assignment_type").unwrap_or_default();
    let raw_cat: String = r.get("assessment_category");

    let Some((student_id, class_id)) =
        resolve_canonical_student_and_class(pool, tenant_id, raw_student_id, hint_class_id).await
    else {
        return;
    };
    let Some((subject_id, subject_code, subject_name)) =
        resolve_canonical_subject(pool, tenant_id, hint_subject_id, desc.as_deref()).await
    else {
        return;
    };

    let category = normalize_assessment_category(Some(&raw_cat), Some(&title));
    let scaled_score = ((raw_score as f64) * 100.0 / (max_score as f64)).clamp(0.0, 100.0);
    let method = if asg_type.eq_ignore_ascii_case("HOMEWORK") {
        "PORTFOLIO"
    } else {
        "WRITTEN_TEST"
    };
    let academic_ctx = resolve_active_academic_context(pool, tenant_id).await;

    upsert_pedagogical_source_result(
        pool,
        tenant_id,
        &academic_ctx,
        class_id,
        &subject_code,
        &subject_name,
        "ASSIGNMENT",
        assignment_id,
        &title,
        &category,
        method,
        student_id,
        scaled_score,
        feedback.as_deref(),
    )
    .await;

    if let Err(e) = recompute_student_subject_gradebook(
        pool,
        tenant_id,
        student_id,
        class_id,
        subject_id,
        &subject_code,
        &subject_name,
    )
    .await
    {
        tracing::warn!("sync_assignment_submission_to_gradebook recompute error: {e}");
    }
}

/// Syncs a graded CBT / quiz attempt to `pedagogical_assessments`, `gradebooks`, and `gradebook_snapshots`.
pub async fn sync_quiz_attempt_to_gradebook(
    pool: &PgPool,
    tenant_id: Uuid,
    attempt_id: Uuid,
) {
    ensure_grade_sync_schema(pool).await;

    let row = sqlx::query(
        r#"
        SELECT qa.student_id,
               qa.score,
               qa.total_points,
               q.id AS quiz_id,
               q.title,
               q.description,
               q.class_id,
               q.subject_id,
               q.max_score,
               COALESCE(NULLIF(TRIM(q.assessment_category), ''), 'FORMATIVE') AS assessment_category
        FROM quiz_attempts qa
        JOIN quizzes q ON q.id = qa.quiz_id
        WHERE qa.id = $1 AND qa.tenant_id = $2
        LIMIT 1
        "#,
    )
    .bind(attempt_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let Some(r) = row else { return };
    let raw_score_opt: Option<i32> = r.try_get("score").ok().flatten();
    let Some(raw_score) = raw_score_opt else { return };

    let raw_student_id: Uuid = r.get("student_id");
    let hint_class_id: Option<Uuid> = r.try_get("class_id").ok().flatten();
    let hint_subject_id: Option<Uuid> = r.try_get("subject_id").ok().flatten();
    let desc: Option<String> = r.try_get("description").ok().flatten();
    let title: String = r.get("title");
    let quiz_id: Uuid = r.get("quiz_id");
    let total_pts: i32 = r.try_get::<Option<i32>, _>("total_points").ok().flatten().unwrap_or(0);
    let max_sc: i32 = r.try_get::<Option<i32>, _>("max_score").ok().flatten().unwrap_or(0);
    let eff_max = if total_pts > 0 {
        total_pts
    } else if max_sc > 0 {
        max_sc
    } else {
        100
    };
    let raw_cat: String = r.get("assessment_category");

    let Some((student_id, class_id)) =
        resolve_canonical_student_and_class(pool, tenant_id, raw_student_id, hint_class_id).await
    else {
        return;
    };
    let Some((subject_id, subject_code, subject_name)) =
        resolve_canonical_subject(pool, tenant_id, hint_subject_id, desc.as_deref()).await
    else {
        return;
    };

    let category = normalize_assessment_category(Some(&raw_cat), Some(&title));
    let scaled_score = ((raw_score as f64) * 100.0 / (eff_max as f64)).clamp(0.0, 100.0);
    let academic_ctx = resolve_active_academic_context(pool, tenant_id).await;

    upsert_pedagogical_source_result(
        pool,
        tenant_id,
        &academic_ctx,
        class_id,
        &subject_code,
        &subject_name,
        "QUIZ",
        quiz_id,
        &title,
        &category,
        "WRITTEN_TEST",
        student_id,
        scaled_score,
        None,
    )
    .await;

    if let Err(e) = recompute_student_subject_gradebook(
        pool,
        tenant_id,
        student_id,
        class_id,
        subject_id,
        &subject_code,
        &subject_name,
    )
    .await
    {
        tracing::warn!("sync_quiz_attempt_to_gradebook recompute error: {e}");
    }
}

/// Retroactively syncs all already-graded assignment submissions and CBT quiz attempts
/// for the tenant (optionally scoped by class_id or subject_id) into `pedagogical_assessments`,
/// `gradebooks`, `gradebook_entries`, and `gradebook_snapshots`. Returns count of synced records.
pub async fn sync_all_graded_sources(
    pool: &PgPool,
    tenant_id: Uuid,
    filter_class_id: Option<Uuid>,
    filter_subject_id: Option<Uuid>,
) -> usize {
    ensure_grade_sync_schema(pool).await;
    let mut synced_count = 0usize;

    let sub_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT sub.id
        FROM assignment_submissions sub
        JOIN assignments a ON a.id = sub.assignment_id
        WHERE a.tenant_id = $1
          AND a.deleted_at IS NULL
          AND (sub.status ILIKE 'graded' OR sub.score IS NOT NULL)
          AND ($2::uuid IS NULL OR a.class_id = $2)
          AND ($3::uuid IS NULL OR a.subject_id = $3 OR a.subject_id IS NULL)
        ORDER BY sub.updated_at DESC
        LIMIT 2000
        "#,
    )
    .bind(tenant_id)
    .bind(filter_class_id)
    .bind(filter_subject_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for sid in sub_ids {
        sync_assignment_submission_to_gradebook(pool, tenant_id, sid).await;
        synced_count += 1;
    }

    let attempt_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT qa.id
        FROM quiz_attempts qa
        JOIN quizzes q ON q.id = qa.quiz_id
        WHERE qa.tenant_id = $1
          AND q.tenant_id = $1
          AND q.deleted_at IS NULL
          AND qa.status ILIKE 'graded'
          AND qa.score IS NOT NULL
          AND ($2::uuid IS NULL OR q.class_id = $2)
          AND ($3::uuid IS NULL OR q.subject_id = $3 OR q.subject_id IS NULL)
        ORDER BY qa.updated_at DESC
        LIMIT 2000
        "#,
    )
    .bind(tenant_id)
    .bind(filter_class_id)
    .bind(filter_subject_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for aid in attempt_ids {
        sync_quiz_attempt_to_gradebook(pool, tenant_id, aid).await;
        synced_count += 1;
    }

    synced_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_assessment_category_explicit_and_title_inference() {
        assert_eq!(normalize_assessment_category(Some("ats"), None), "ATS");
        assert_eq!(normalize_assessment_category(Some("PAS"), None), "SAS");
        assert_eq!(normalize_assessment_category(Some("m2"), None), "SUMMATIVE_M2");
        assert_eq!(normalize_assessment_category(None, Some("UTS Bahasa Indonesia 7 Des")), "ATS");
        assert_eq!(normalize_assessment_category(None, Some("PAS Ganjil Matematika")), "SAS");
        assert_eq!(normalize_assessment_category(None, Some("Latihan Harian Bab 1")), "FORMATIVE");
    }

    #[test]
    fn test_merge_auto_formative_notes_preserves_manual_notes() {
        let merged = merge_auto_formative_notes(
            Some("[Auto-Formatif: Rata-rata 70 (1 tugas)]\nAktif berdiskusi di kelas"),
            Some("[Auto-Formatif: Rata-rata 85 (2 tugas)]".to_string()),
        );
        assert_eq!(
            merged.as_deref(),
            Some("[Auto-Formatif: Rata-rata 85 (2 tugas)]\nAktif berdiskusi di kelas")
        );
    }
}
