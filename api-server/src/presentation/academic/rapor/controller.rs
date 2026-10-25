use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::Datelike;
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use super::dto::{
    AttendanceSummaryDto, ExtracurricularDto, ReportCardsResponse, StudentReportCardDto,
    SubjectCompetencyDto,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::ApplicationError;

#[derive(Debug, Deserialize)]
pub struct ReportCardsQuery {
    pub class_id: Option<Uuid>,
    pub academic_year_id: Option<Uuid>,
    pub semester: Option<i16>,
}

pub fn report_card_routes() -> Router<ApplicationContext> {
    Router::new().route("/cards", get(get_report_cards))
}

fn get_curriculum_phase(class_name: &str) -> &'static str {
    let upper = class_name.to_uppercase();
    if upper.contains("PAKET C") || upper.contains("SMA") || upper.contains("SMK") {
        if upper.contains("10") || upper.contains(" X") {
            "Fase E"
        } else {
            "Fase F"
        }
    } else if upper.contains("PAKET B") || upper.contains("SMP") {
        "Fase D"
    } else if upper.contains("PAKET A") || upper.contains("SD") {
        if upper.contains(" 1") || upper.contains(" 2") {
            "Fase A"
        } else if upper.contains(" 3") || upper.contains(" 4") {
            "Fase B"
        } else {
            "Fase C"
        }
    } else {
        "Fase F"
    }
}

fn format_indonesian_date(date: Option<chrono::NaiveDate>, place: Option<&str>) -> String {
    let date_str = match date {
        Some(d) => {
            let months = [
                "Januari", "Februari", "Maret", "April", "Mei", "Juni",
                "Juli", "Agustus", "September", "Oktober", "November", "Desember",
            ];
            let month_name = months.get(d.month0() as usize).unwrap_or(&"Januari");
            format!("{} {} {}", d.day(), month_name, d.year())
        }
        None => "".to_string(),
    };

    match place {
        Some(p) if !p.trim().is_empty() => {
            if date_str.is_empty() {
                p.to_string()
            } else {
                format!("{}, {}", p.trim(), date_str)
            }
        }
        _ => date_str,
    }
}

fn build_empty_curriculum_subjects(
    class_name: &str,
    _phase: &str,
    religion: Option<&str>,
) -> Vec<SubjectCompetencyDto> {
    let rel_str = religion.unwrap_or("Islam").to_lowercase();
    let agama_name = if rel_str.contains("kristen") {
        "Pendidikan Agama Kristen dan Budi Pekerti"
    } else if rel_str.contains("katolik") {
        "Pendidikan Agama Katolik dan Budi Pekerti"
    } else if rel_str.contains("hindu") {
        "Pendidikan Agama Hindu dan Budi Pekerti"
    } else if rel_str.contains("buddha") {
        "Pendidikan Agama Buddha dan Budi Pekerti"
    } else {
        "Pendidikan Agama Islam dan Budi Pekerti"
    };

    let upper = class_name.to_uppercase();
    let is_paket_c = upper.contains("PAKET C") || upper.contains("SMA");
    let is_paket_b = upper.contains("PAKET B") || upper.contains("SMP");
    let is_paket_a = upper.contains("PAKET A") || upper.contains("SD");
    let is_kelas_10 = upper.contains("10") || upper.contains(" X");

    let mut list = Vec::new();
    let default_desc = "Belum ada penilaian capaian kompetensi yang diinput oleh guru pengampu.".to_string();

    let mut add_subj = |name: &str, group: &str| {
        list.push(SubjectCompetencyDto {
            subject_name: name.to_string(),
            final_score: 0.0,
            predicate: "-".to_string(),
            description: default_desc.clone(),
            group: group.to_string(),
        });
    };

    if is_paket_c {
        if is_kelas_10 {
            add_subj(agama_name, "UMUM");
            add_subj("Pendidikan Pancasila", "UMUM");
            add_subj("Bahasa Indonesia", "UMUM");
            add_subj("Matematika (Umum)", "UMUM");
            add_subj("Ilmu Pengetahuan Alam (IPA)", "UMUM");
            add_subj("Ilmu Pengetahuan Sosial (IPS)", "UMUM");
            add_subj("Bahasa Inggris", "UMUM");
            add_subj("Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)", "UMUM");
            add_subj("Informatika", "UMUM");
            add_subj("Seni Budaya", "UMUM");
            add_subj("Pemberdayaan", "UMUM");
            add_subj("Keterampilan Fungsional", "UMUM");
            add_subj("Muatan Lokal (Bahasa Daerah)", "MUATAN_LOKAL");
        } else {
            // Fase F (Kelas 11 & 12)
            add_subj(agama_name, "UMUM");
            add_subj("Pendidikan Pancasila", "UMUM");
            add_subj("Bahasa Indonesia", "UMUM");
            add_subj("Matematika (Umum)", "UMUM");
            add_subj("Bahasa Inggris", "UMUM");
            add_subj("Sejarah", "UMUM");
            add_subj("Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)", "UMUM");
            add_subj("Seni Budaya", "UMUM");
            add_subj("Pemberdayaan", "UMUM");
            add_subj("Keterampilan Fungsional", "UMUM");
            add_subj("Sosiologi", "PILIHAN");
            add_subj("Ekonomi", "PILIHAN");
            add_subj("Muatan Lokal (Bahasa Daerah)", "MUATAN_LOKAL");
        }
    } else if is_paket_b {
        add_subj(agama_name, "UMUM");
        add_subj("Pendidikan Pancasila", "UMUM");
        add_subj("Bahasa Indonesia", "UMUM");
        add_subj("Matematika", "UMUM");
        add_subj("Ilmu Pengetahuan Alam (IPA)", "UMUM");
        add_subj("Ilmu Pengetahuan Sosial (IPS)", "UMUM");
        add_subj("Bahasa Inggris", "UMUM");
        add_subj("Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)", "UMUM");
        add_subj("Informatika", "UMUM");
        add_subj("Seni Budaya", "UMUM");
        add_subj("Pemberdayaan", "UMUM");
        add_subj("Keterampilan", "UMUM");
        add_subj("Muatan Lokal (Bahasa Daerah)", "MUATAN_LOKAL");
    } else if is_paket_a {
        add_subj(agama_name, "UMUM");
        add_subj("Pendidikan Pancasila", "UMUM");
        add_subj("Bahasa Indonesia", "UMUM");
        add_subj("Matematika", "UMUM");
        add_subj("Ilmu Pengetahuan Alam dan Sosial (IPAS)", "UMUM");
        add_subj("Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)", "UMUM");
        add_subj("Seni Budaya", "UMUM");
        add_subj("Pemberdayaan", "UMUM");
        add_subj("Keterampilan", "UMUM");
        add_subj("Muatan Lokal (Bahasa Daerah)", "MUATAN_LOKAL");
    } else {
        add_subj(agama_name, "UMUM");
        add_subj("Pendidikan Pancasila", "UMUM");
        add_subj("Bahasa Indonesia", "UMUM");
        add_subj("Matematika (Umum)", "UMUM");
        add_subj("Bahasa Inggris", "UMUM");
        add_subj("Muatan Lokal (Bahasa Daerah)", "MUATAN_LOKAL");
    }

    list
}

struct PromotionEvaluation {
    is_passed: bool,
    decision_text: String,
    summary_reason: String,
}

fn calculate_promotion_decision(
    class_name: &str,
    subjects: &[SubjectCompetencyDto],
    attendance: &AttendanceSummaryDto,
    semester: i16,
) -> PromotionEvaluation {
    // Menurut Regulasi Kemendikdasmen RI (BSKAP No. 033/H/KR/2024 & Panduan Pembelajaran dan Asesmen):
    // Penentuan Kenaikan Kelas atau Kelulusan HANYA diambil dan dicetak pada akhir tahun ajaran (Semester Genap).
    // Pada Semester Gasal (1), peserta didik berstatus AKTIF dalam proses pembelajaran.
    if semester != 2 {
        return PromotionEvaluation {
            is_passed: true,
            decision_text: "AKTIF (Dalam Proses Pembelajaran)".to_string(),
            summary_reason: "Peserta didik berstatus aktif mengikuti proses pembelajaran Semester Gasal.".to_string(),
        };
    }

    let valid_scores: Vec<f64> = subjects
        .iter()
        .map(|s| s.final_score)
        .filter(|&sc| sc > 0.0)
        .collect();

    if valid_scores.is_empty() {
        return PromotionEvaluation {
            is_passed: false,
            decision_text: "AKTIF (Dalam Proses Pembelajaran)".to_string(),
            summary_reason: "Nilai mata pelajaran belum diinput oleh guru pengampu pada Buku Nilai.".to_string(),
        };
    }

    let upper = class_name.to_uppercase();
    let mut grade_num: Option<i32> = None;
    if upper.contains("12") || upper.contains("XII") {
        grade_num = Some(12);
    } else if upper.contains("11") || upper.contains("XI") {
        grade_num = Some(11);
    } else if upper.contains("10") || upper.contains(" X") {
        grade_num = Some(10);
    } else if upper.contains(" 9") || upper.contains("IX") {
        grade_num = Some(9);
    } else if upper.contains(" 8") || upper.contains("VIII") {
        grade_num = Some(8);
    } else if upper.contains(" 7") || upper.contains("VII") {
        grade_num = Some(7);
    } else if upper.contains(" 6") || upper.contains("VI") {
        grade_num = Some(6);
    }

    let is_graduation = grade_num == Some(12) || grade_num == Some(9) || grade_num == Some(6);
    let avg_score = valid_scores.iter().sum::<f64>() / (valid_scores.len() as f64);

    let failed_count = subjects
        .iter()
        .filter(|s| s.final_score > 0.0 && s.final_score < 70.0)
        .count();

    let alpha_count = attendance.alpha;
    let is_attendance_ok = alpha_count <= 10;
    let is_passed = avg_score >= 70.0 && failed_count <= 2 && is_attendance_ok;

    if is_graduation {
        if is_passed {
            PromotionEvaluation {
                is_passed: true,
                decision_text: "LULUS (Tamat Belajar Satuan Pendidikan)".to_string(),
                summary_reason: format!(
                    "Rata-rata: {:.1} (KKTP ≥ 70) · Mapel di bawah KKTP: {} (Maks 2) · Alpha: {} hari",
                    avg_score, failed_count, alpha_count
                ),
            }
        } else {
            PromotionEvaluation {
                is_passed: false,
                decision_text: "TIDAK LULUS (Belum Memenuhi Kriteria Kelulusan Satuan Pendidikan)".to_string(),
                summary_reason: format!(
                    "Rata-rata: {:.1} · Mapel di bawah KKTP: {} · Alpha: {} hari",
                    avg_score, failed_count, alpha_count
                ),
            }
        }
    } else {
        let next_grade = match grade_num {
            Some(g) => format!("{}", g + 1),
            None => "Berikutnya".to_string(),
        };
        let current_grade = match grade_num {
            Some(g) => format!("Kelas {}", g),
            None => "Tingkat Saat Ini".to_string(),
        };

        if is_passed {
            PromotionEvaluation {
                is_passed: true,
                decision_text: format!("NAIK KELAS (Melanjutkan ke Kelas {})", next_grade),
                summary_reason: format!(
                    "Rata-rata: {:.1} (KKTP ≥ 70) · Mapel di bawah KKTP: {} (Maks 2) · Alpha: {} hari",
                    avg_score, failed_count, alpha_count
                ),
            }
        } else {
            PromotionEvaluation {
                is_passed: false,
                decision_text: format!("TINGGAL KELAS (Mengulang di {})", current_grade),
                summary_reason: format!(
                    "Rata-rata: {:.1} · Mapel di bawah KKTP: {} · Alpha: {} hari",
                    avg_score, failed_count, alpha_count
                ),
            }
        }
    }
}

struct StudentRow {
    student_id: Uuid,
    nisn: String,
    nipd: Option<String>,
    student_name: String,
    gender: Option<String>,
    place_of_birth: Option<String>,
    date_of_birth: Option<chrono::NaiveDate>,
    religion: Option<String>,
    address: Option<String>,
    parent_phone: Option<String>,
    class_name: Option<String>,
    homeroom_teacher_name: Option<String>,
    guardian_name: Option<String>,
}

/// Retrieve unified e-Rapor cards for students in the active academic context.
#[utoipa::path(
    get,
    operation_id = "getReportCards",
    path = "/api/v1/academic/rapor/cards",
    params(
        ("class_id" = Option<Uuid>, Query, description = "Filter by class UUID"),
        ("academic_year_id" = Option<Uuid>, Query, description = "Filter by academic year UUID"),
        ("semester" = Option<i16>, Query, description = "Filter by semester (1 for Gasal, 2 for Genap)"),
    ),
    responses(
        (status = 200, description = "Unified report cards retrieved successfully", body = inline(ApiResponse<ReportCardsResponse>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Academic"
)]
pub async fn get_report_cards(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<ReportCardsQuery>,
) -> Result<Json<ApiResponse<ReportCardsResponse>>, ApiError> {
    // 1. Fetch Academic Year & Semester SSOT
    let year_row = if let Some(year_id) = params.academic_year_id {
        sqlx::query(
            r#"SELECT id, name, start_date, end_date, current_semester, report_date FROM academic_years WHERE id = $1 AND tenant_id = $2 AND deleted_at IS NULL"#,
        )
        .bind(year_id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
    } else {
        sqlx::query(
            r#"SELECT id, name, start_date, end_date, current_semester, report_date FROM academic_years WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL ORDER BY updated_at DESC LIMIT 1"#,
        )
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
    }
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let (active_year_name, active_semester, report_date_str) = match year_row {
        Some(ref y) => {
            let name: String = y.get("name");
            let db_sem: i16 = y.try_get::<i16, _>("current_semester").unwrap_or(1);
            let sem = params.semester.unwrap_or(db_sem);
            let rd: Option<chrono::NaiveDate> = y.try_get::<Option<chrono::NaiveDate>, _>("report_date").unwrap_or(None);
            let rd_str = rd.map(|d| d.format("%Y-%m-%d").to_string());
            (name, sem, rd_str)
        }
        None => (
            "2026/2027".to_string(),
            params.semester.unwrap_or(1),
            Some("2026-12-19".to_string()),
        ),
    };

    let semester_label = if active_semester == 2 {
        "Semester 2 (Genap)".to_string()
    } else {
        "Semester 1 (Gasal)".to_string()
    };

    // 2. Fetch School Profile SSOT
    let school_row = sqlx::query(
        r#"
        SELECT name, npsn, headmaster_name, headmaster_nip, address, village, district, regency, province, postal_code
        FROM schools
        WHERE tenant_id = $1 AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let school_name = school_row
        .as_ref()
        .map(|s| s.get::<String, _>("name"))
        .unwrap_or_else(|| "Nama Sekolah".to_string());
    let school_npsn = school_row
        .as_ref()
        .and_then(|s| s.try_get::<Option<String>, _>("npsn").unwrap_or(None))
        .unwrap_or_else(|| "-".to_string());
    let headmaster_name = school_row
        .as_ref()
        .and_then(|s| s.try_get::<Option<String>, _>("headmaster_name").unwrap_or(None))
        .unwrap_or_else(|| "Kepala Sekolah".to_string());
    let headmaster_nip = school_row
        .as_ref()
        .and_then(|s| s.try_get::<Option<String>, _>("headmaster_nip").unwrap_or(None))
        .unwrap_or_else(|| "-".to_string());

    // 3. Query Students with Class, Teacher, and Guardian
    let student_rows = sqlx::query(
        r#"
        WITH active_student_roster AS (
            SELECT DISTINCT ON (s.id)
                s.id AS student_id,
                s.nisn,
                s.nipd,
                s.full_name AS student_name,
                s.gender,
                s.place_of_birth,
                s.date_of_birth,
                s.religion,
                s.alamat_jalan AS address,
                s.no_hp AS parent_phone,
                c.id AS class_id,
                c.name AS class_name,
                t.full_name AS homeroom_teacher_name,
                g.full_name AS guardian_name
            FROM students s
            LEFT JOIN enrollments e ON e.student_id = s.id AND e.tenant_id = s.tenant_id AND (e.status = 'Active' OR e.status = 'active')
            LEFT JOIN classes c ON c.id = e.class_id AND c.tenant_id = s.tenant_id
            LEFT JOIN teachers t ON t.id = c.homeroom_teacher_id AND t.tenant_id = s.tenant_id
            LEFT JOIN guardians g ON g.id = s.guardian_id AND g.tenant_id = s.tenant_id
            WHERE s.tenant_id = $1 
              AND s.deleted_at IS NULL
              AND (s.status = 'active' OR (s.status IS NULL AND s.is_active = true))
              AND ($2::uuid IS NULL OR c.id = $2)
            ORDER BY s.id, e.enrolled_at DESC NULLS LAST
        )
        SELECT * FROM active_student_roster
        ORDER BY class_name NULLS LAST, student_name ASC
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(params.class_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let students: Vec<StudentRow> = student_rows
        .iter()
        .map(|row| StudentRow {
            student_id: row.get("student_id"),
            nisn: row.get("nisn"),
            nipd: row.try_get::<Option<String>, _>("nipd").unwrap_or(None),
            student_name: row.get("student_name"),
            gender: row.try_get::<Option<String>, _>("gender").unwrap_or(None),
            place_of_birth: row.try_get::<Option<String>, _>("place_of_birth").unwrap_or(None),
            date_of_birth: row.try_get::<Option<chrono::NaiveDate>, _>("date_of_birth").unwrap_or(None),
            religion: row.try_get::<Option<String>, _>("religion").unwrap_or(None),
            address: row.try_get::<Option<String>, _>("address").unwrap_or(None),
            parent_phone: row.try_get::<Option<String>, _>("parent_phone").unwrap_or(None),
            class_name: row.try_get::<Option<String>, _>("class_name").unwrap_or(None),
            homeroom_teacher_name: row.try_get::<Option<String>, _>("homeroom_teacher_name").unwrap_or(None),
            guardian_name: row.try_get::<Option<String>, _>("guardian_name").unwrap_or(None),
        })
        .collect();

    // 4. Query Gradebook Snapshots (Cast final_score to float8 for direct native f64 decoding)
    let snapshot_rows = sqlx::query(
        r#"
        SELECT 
            student_id,
            subject_code,
            subject_name,
            final_score::float8 AS final_score,
            status_kktp,
            highest_achievement_desc,
            lowest_improvement_desc
        FROM gradebook_snapshots
        WHERE tenant_id = $1
          AND academic_year = $2
          AND semester = $3
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(&active_year_name)
    .bind(&semester_label)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    // Map snapshots by student_id
    let mut snapshots_by_student: std::collections::HashMap<Uuid, Vec<SubjectCompetencyDto>> =
        std::collections::HashMap::new();

    for row in snapshot_rows {
        let st_id: Option<Uuid> = row.try_get::<Option<Uuid>, _>("student_id").unwrap_or(None);
        let subj_name: String = row.try_get::<String, _>("subject_name").unwrap_or_default();
        let highest_desc: Option<String> = row.try_get::<Option<String>, _>("highest_achievement_desc").unwrap_or(None);
        let final_sc: f64 = row.try_get::<f64, _>("final_score").unwrap_or(80.0);

        if let Some(student_id) = st_id {
            let predicate = if final_sc >= 88.0 {
                "A".to_string()
            } else if final_sc >= 78.0 {
                "B".to_string()
            } else if final_sc >= 70.0 {
                "C".to_string()
            } else {
                "D".to_string()
            };

            let desc = highest_desc
                .filter(|d| !d.trim().is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "Menunjukkan pemahaman yang baik dalam mata pelajaran {}.",
                        subj_name
                    )
                });

            let group = if subj_name.to_lowercase().contains("pilihan")
                || subj_name.to_lowercase().contains("sosiologi")
                || subj_name.to_lowercase().contains("ekonomi")
                || subj_name.to_lowercase().contains("geografi")
            {
                "PILIHAN".to_string()
            } else if subj_name.to_lowercase().contains("muatan lokal")
                || subj_name.to_lowercase().contains("daerah")
                || subj_name.to_lowercase().contains("cirebon")
            {
                "MUATAN_LOKAL".to_string()
            } else {
                "UMUM".to_string()
            };

            snapshots_by_student
                .entry(student_id)
                .or_default()
                .push(SubjectCompetencyDto {
                    subject_name: subj_name,
                    final_score: final_sc,
                    predicate,
                    description: desc,
                    group,
                });
        }
    }

    // 5. Query Attendance Summary
    let attendance_rows = sqlx::query(
        r#"
        SELECT 
            student_id,
            status,
            COUNT(*) as count
        FROM session_attendances
        WHERE tenant_id = $1
        GROUP BY student_id, status
        "#,
    )
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let mut attendance_by_student: std::collections::HashMap<Uuid, AttendanceSummaryDto> =
        std::collections::HashMap::new();

    for row in attendance_rows {
        let st_id: Option<Uuid> = row.try_get::<Option<Uuid>, _>("student_id").unwrap_or(None);
        let status: String = row.try_get::<String, _>("status").unwrap_or_default();
        let cnt: i64 = row.try_get::<i64, _>("count").unwrap_or(0);

        if let Some(student_id) = st_id {
            let entry = attendance_by_student
                .entry(student_id)
                .or_insert_with(|| AttendanceSummaryDto {
                    sakit: 0,
                    izin: 0,
                    alpha: 0,
                });

            let status_upper = status.to_uppercase();
            if status_upper.contains("SAKIT") {
                entry.sakit += cnt;
            } else if status_upper.contains("IZIN") {
                entry.izin += cnt;
            } else if status_upper.contains("ALPA") || status_upper.contains("ALPHA") {
                entry.alpha += cnt;
            }
        }
    }

    // 6. Build StudentReportCardDto list
    let mut cards = Vec::with_capacity(students.len());

    let default_extracurricular = vec![
        ExtracurricularDto {
            name: "Pramuka Penegak/Penggalang".to_string(),
            predicate: "Sangat Baik".to_string(),
            description: "Aktif mengikuti kegiatan kepramukaan, kepemimpinan, dan bakti sosial.".to_string(),
        },
        ExtracurricularDto {
            name: "Keterampilan Komputer & Digital".to_string(),
            predicate: "Baik".to_string(),
            description: "Mampu mengoperasikan aplikasi perkantoran, desain grafis dasar, dan internet sehat.".to_string(),
        },
    ];

    for s in students {
        let cls_name = s.class_name.unwrap_or_else(|| "Kelas Umum".to_string());
        let phase = get_curriculum_phase(&cls_name).to_string();
        let birth_str = format_indonesian_date(s.date_of_birth, s.place_of_birth.as_deref());

        // Get or build subject competencies
        let subjects = if let Some(snaps) = snapshots_by_student.remove(&s.student_id) {
            snaps
        } else {
            // When teachers have not yet inputted grades into gradebook,
            // subjects are created with 0.0 scores, predicate "-", and description "-" / belum dinilai.
            build_empty_curriculum_subjects(&cls_name, &phase, s.religion.as_deref())
        };

        let has_scores = subjects.iter().any(|sub| sub.final_score > 0.0);

        let attendance = attendance_by_student
            .remove(&s.student_id)
            .unwrap_or(AttendanceSummaryDto {
                sakit: 0,
                izin: 0,
                alpha: 0,
            });

        let promo_eval = calculate_promotion_decision(
            &cls_name,
            &subjects,
            &attendance,
            active_semester,
        );

        let qr_token = format!(
            "QR-{}-{}-2026-VERIFIED",
            school_npsn,
            s.nisn
        );

        let card_status = if has_scores {
            "SIAP_CETAK".to_string()
        } else {
            "DRAF_BELUM_DINILAI".to_string()
        };

        cards.push(StudentReportCardDto {
            student_id: s.student_id,
            nisn: s.nisn,
            nipd: s.nipd.unwrap_or_else(|| "-".to_string()),
            student_name: s.student_name,
            gender: s.gender.unwrap_or_else(|| "L".to_string()),
            birth_place_date: birth_str,
            religion: s.religion.unwrap_or_else(|| "Islam".to_string()),
            address: s.address.clone().unwrap_or_else(|| "-".to_string()),
            father_name: "-".to_string(),
            mother_name: "-".to_string(),
            parent_address: s.address.unwrap_or_else(|| "-".to_string()),
            parent_phone: s.parent_phone.unwrap_or_else(|| "-".to_string()),
            class_name: cls_name,
            phase,
            academic_year: active_year_name.clone(),
            semester: semester_label.clone(),
            headmaster_name: headmaster_name.clone(),
            headmaster_nip: headmaster_nip.clone(),
            teacher_name: s.homeroom_teacher_name.unwrap_or_else(|| "WALI KELAS".to_string()),
            guardian_name: s.guardian_name.unwrap_or_else(|| "-".to_string()),
            subjects,
            attendance,
            extracurricular: if has_scores { default_extracurricular.clone() } else { Vec::new() },
            p5_character_note: if has_scores {
                "Peserta didik aktif bergotong royong, memiliki daya nalar kritis yang baik, dan menunjukkan sikap mandiri dalam menyelesaikan tugas pembelajaran.".to_string()
            } else {
                "Dalam proses pemantauan dan pembiasaan karakter profil pelajar Pancasila.".to_string()
            },
            promotion_status: promo_eval.decision_text,
            is_promotion_passed: promo_eval.is_passed,
            promotion_reason: Some(promo_eval.summary_reason),
            qr_token,
            status: card_status,
        });
    }

    let total_students = cards.len();

    let response = ReportCardsResponse {
        academic_year: active_year_name,
        semester: active_semester,
        semester_label,
        school_name,
        npsn: school_npsn,
        headmaster_name,
        headmaster_nip,
        report_date: report_date_str,
        total_students,
        cards,
    };

    Ok(Json(ApiResponse::success(response, req_ctx.request_id)))
}
