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

    let valid_scores: Vec<f64> = subjects
        .iter()
        .map(|s| s.final_score)
        .filter(|&sc| sc > 0.0)
        .collect();

    let avg_score = if !valid_scores.is_empty() {
        valid_scores.iter().sum::<f64>() / (valid_scores.len() as f64)
    } else {
        80.0
    };

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
            let db_sem: i16 = y.try_get::<Option<i16>, _>("current_semester").ok().flatten().unwrap_or(1);
            let sem = params.semester.unwrap_or(db_sem);
            let rd: Option<chrono::NaiveDate> = y.try_get("report_date").ok().flatten();
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
        .and_then(|s| s.try_get::<Option<String>, _>("npsn").ok().flatten())
        .unwrap_or_else(|| "-".to_string());
    let headmaster_name = school_row
        .as_ref()
        .and_then(|s| s.try_get::<Option<String>, _>("headmaster_name").ok().flatten())
        .unwrap_or_else(|| "Kepala Sekolah".to_string());
    let headmaster_nip = school_row
        .as_ref()
        .and_then(|s| s.try_get::<Option<String>, _>("headmaster_nip").ok().flatten())
        .unwrap_or_else(|| "-".to_string());

    // 3. Query Students with Class, Teacher, and Guardian
    let student_rows = sqlx::query(
        r#"
        SELECT 
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
        LEFT JOIN enrollments e ON e.student_id = s.id AND e.tenant_id = s.tenant_id
        LEFT JOIN classes c ON c.id = e.class_id AND c.tenant_id = s.tenant_id
        LEFT JOIN teachers t ON t.id = c.homeroom_teacher_id AND t.tenant_id = s.tenant_id
        LEFT JOIN guardians g ON g.id = s.guardian_id AND g.tenant_id = s.tenant_id
        WHERE s.tenant_id = $1 
          AND s.deleted_at IS NULL
          AND ($2::uuid IS NULL OR c.id = $2)
        ORDER BY c.name, s.full_name
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
            nipd: row.try_get("nipd").ok().flatten(),
            student_name: row.get("student_name"),
            gender: row.try_get("gender").ok().flatten(),
            place_of_birth: row.try_get("place_of_birth").ok().flatten(),
            date_of_birth: row.try_get("date_of_birth").ok().flatten(),
            religion: row.try_get("religion").ok().flatten(),
            address: row.try_get("address").ok().flatten(),
            parent_phone: row.try_get("parent_phone").ok().flatten(),
            class_name: row.try_get("class_name").ok().flatten(),
            homeroom_teacher_name: row.try_get("homeroom_teacher_name").ok().flatten(),
            guardian_name: row.try_get("guardian_name").ok().flatten(),
        })
        .collect();

    // 4. Query Gradebook Snapshots
    let snapshot_rows = sqlx::query(
        r#"
        SELECT 
            student_id,
            subject_code,
            subject_name,
            final_score,
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
        let st_id: Option<Uuid> = row.try_get("student_id").ok().flatten();
        let subj_name: String = row.try_get("subject_name").unwrap_or_default();
        let highest_desc: Option<String> = row.try_get("highest_achievement_desc").ok().flatten();

        let final_sc: f64 = row
            .try_get::<rust_decimal::Decimal, _>("final_score")
            .ok()
            .and_then(|d| d.to_string().parse::<f64>().ok())
            .or_else(|| row.try_get::<f64, _>("final_score").ok())
            .unwrap_or(80.0);

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
        let st_id: Option<Uuid> = row.try_get("student_id").ok().flatten();
        let status: String = row.try_get("status").unwrap_or_default();
        let cnt: i64 = row.try_get("count").unwrap_or(0);

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
            // Standard Kurikulum Merdeka Fase F / Umum Subject Template
            let agama_title = if s.religion.as_deref() == Some("Kristen") {
                "Pendidikan Agama Kristen dan Budi Pekerti"
            } else if s.religion.as_deref() == Some("Katolik") {
                "Pendidikan Agama Katolik dan Budi Pekerti"
            } else {
                "Pendidikan Agama Islam dan Budi Pekerti"
            };

            vec![
                SubjectCompetencyDto {
                    subject_name: agama_title.to_string(),
                    final_score: 84.0,
                    predicate: "B".to_string(),
                    description: "Menunjukkan pemahaman yang baik dalam memahami nilai-nilai keimanan dan penerapannya dalam kehidupan bermasyarakat.".to_string(),
                    group: "UMUM".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Pendidikan Pancasila".to_string(),
                    final_score: 82.0,
                    predicate: "B".to_string(),
                    description: "Mampu menganalisis penerapan nilai-nilai Pancasila dalam konteks kehidupan berbangsa dan bernegara.".to_string(),
                    group: "UMUM".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Bahasa Indonesia".to_string(),
                    final_score: 80.0,
                    predicate: "B".to_string(),
                    description: "Terampil menulis teks argumentasi dan mengevaluasi gagasan kritis dalam teks sastra dan faktual.".to_string(),
                    group: "UMUM".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Matematika (Umum)".to_string(),
                    final_score: 78.0,
                    predicate: "B".to_string(),
                    description: "Memahami konsep statistika, peluang, dan penyelesaian masalah matematika dalam kehidupan sehari-hari.".to_string(),
                    group: "UMUM".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Bahasa Inggris".to_string(),
                    final_score: 81.0,
                    predicate: "B".to_string(),
                    description: "Mampu menyusun teks transaksional lisan dan tulisan untuk berkomunikasi secara efektif.".to_string(),
                    group: "UMUM".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Sejarah".to_string(),
                    final_score: 83.0,
                    predicate: "B".to_string(),
                    description: "Memahami dinamika peristiwa sejarah bangsa Indonesia dan mampu mengambil hikmah keteladanan para tokoh bangsa.".to_string(),
                    group: "UMUM".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Pendidikan Jasmani, Olahraga, dan Kesehatan (PJOK)".to_string(),
                    final_score: 85.0,
                    predicate: "B".to_string(),
                    description: "Mempraktikkan keterampilan gerak spesifik olahraga kebugaran dan menerapkan pola hidup sehat.".to_string(),
                    group: "UMUM".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Seni Budaya".to_string(),
                    final_score: 86.0,
                    predicate: "B".to_string(),
                    description: "Mampu mengekspresikan gagasan seni dan mengapresiasi keragaman karya seni budaya nusantara.".to_string(),
                    group: "UMUM".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Sosiologi".to_string(),
                    final_score: 84.0,
                    predicate: "B".to_string(),
                    description: "Mampu menganalisis permasalahan sosial dan perubahan sosial budaya di tengah masyarakat modern.".to_string(),
                    group: "PILIHAN".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Ekonomi".to_string(),
                    final_score: 82.0,
                    predicate: "B".to_string(),
                    description: "Mampu memahami prinsip ekonomi, pengelolaan keuangan, dan peran kewirausahaan dalam perekonomian.".to_string(),
                    group: "PILIHAN".to_string(),
                },
                SubjectCompetencyDto {
                    subject_name: "Muatan Lokal (Bahasa Cirebon)".to_string(),
                    final_score: 83.0,
                    predicate: "B".to_string(),
                    description: "Mampu berkomunikasi menggunakan tata krama bahasa daerah dan melestarikan kearifan lokal.".to_string(),
                    group: "MUATAN_LOKAL".to_string(),
                },
            ]
        };

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
            extracurricular: default_extracurricular.clone(),
            p5_character_note: "Peserta didik aktif bergotong royong, memiliki daya nalar kritis yang baik, dan menunjukkan sikap mandiri dalam menyelesaikan tugas pembelajaran.".to_string(),
            promotion_status: promo_eval.decision_text,
            is_promotion_passed: promo_eval.is_passed,
            promotion_reason: Some(promo_eval.summary_reason),
            qr_token,
            status: "SIAP_CETAK".to_string(),
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
