use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use school_core::common::error::ApplicationError;
use school_core::common::error_code::ErrorCode;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};

#[derive(Debug, Deserialize)]
pub struct ReportExportQuery {
    pub format: Option<String>,
    pub class_id: Option<Uuid>,
    pub academic_year: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReportTypeInfo {
    pub id: String,
    pub title: String,
    pub category: String,
    pub description: String,
    pub icon: String,
    pub file_prefix: String,
    pub default_format: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReportHubStats {
    pub total_students: i64,
    pub active_students: i64,
    pub mutated_students: i64,
    pub pip_students: i64,
    pub total_teachers: i64,
    pub total_staff: i64,
    pub total_classes: i64,
    pub total_quizzes: i64,
    pub total_schedules: i64,
    pub total_habits: i64,
    pub total_audit_logs: i64,
}

pub fn report_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/types", get(list_report_types))
        .route("/stats", get(get_report_hub_stats))
        .route("/export/{report_type}", get(export_report))
}

#[utoipa::path(
    get,
    operation_id = "listReportTypes",
    path = "/api/v1/reports/types",
    responses(
        (status = 200, description = "List of exportable institutional report types", body = ApiResponse<Vec<ReportTypeInfo>>),
        (status = 401, description = "Unauthorized")
    ),
    security(("Bearer" = [])),
    tag = "Reports"
)]
pub async fn list_report_types(
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<ReportTypeInfo>>>, ApiError> {
    let types = vec![
        ReportTypeInfo {
            id: "master-dapodik".to_string(),
            title: "Data Master Siswa Dapodik Lengkap".to_string(),
            category: "Kesiswaan & Dapodik".to_string(),
            description: "Buku Induk Master Siswa resmi dengan NISN, NIK, NIPD, Tempat/Tgl Lahir, Rombel, Alamat Lengkap, dan Data Orang Tua.".to_string(),
            icon: "Folder".to_string(),
            file_prefix: "Master_Siswa_Dapodik".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "NISN".to_string(), "NIK".to_string(), "NIPD".to_string(), "Nama Lengkap Siswa".to_string(),
                "Jenis Kelamin".to_string(), "Tempat Lahir".to_string(), "Tanggal Lahir".to_string(),
                "Agama".to_string(), "Rombel".to_string(), "Alamat Jalan".to_string(), "RT".to_string(),
                "RW".to_string(), "Dusun".to_string(), "Kelurahan".to_string(), "Kecamatan".to_string(),
                "Kode Pos".to_string(), "No HP".to_string(), "Nama Ayah".to_string(), "Nama Ibu".to_string(),
                "Nama Wali".to_string(), "Status Dapodik".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "mutasi-siswa".to_string(),
            title: "Buku Induk Mutasi & Siswa Keluar".to_string(),
            category: "Kesiswaan & Dapodik".to_string(),
            description: "Rekapitulasi riwayat mutasi siswa (Pindah Masuk, Keluar, Lulus, Drop Out) beserta nomor SK dan sekolah tujuan.".to_string(),
            icon: "Archive".to_string(),
            file_prefix: "Buku_Induk_Mutasi_Siswa".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "NISN".to_string(), "Nama Siswa".to_string(), "Jenis Kelamin".to_string(),
                "Jenis Mutasi".to_string(), "Tanggal Mutasi".to_string(), "Sekolah Tujuan".to_string(),
                "Nomor SK Mutasi".to_string(), "Rombel Terakhir".to_string(), "Alasan / Keterangan".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "pip-bantuan".to_string(),
            title: "Rekapitulasi Siswa Bantuan PIP (KIP/KPS/PKH)".to_string(),
            category: "Kesiswaan & Dapodik".to_string(),
            description: "Data siswa penerima KIP/KPS/KKS, kelayakan PIP, serta bank penyalur dan nomor rekening simpanan pelajar.".to_string(),
            icon: "GraduationCap".to_string(),
            file_prefix: "Rekap_Bantuan_PIP_KIP".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "NISN".to_string(), "Nama Siswa".to_string(), "Rombel".to_string(), "Penerima KIP".to_string(),
                "Nomor KIP".to_string(), "Nama di KIP".to_string(), "Penerima KPS".to_string(), "Nomor KPS".to_string(),
                "Nomor KKS".to_string(), "Layak PIP".to_string(), "Alasan Layak PIP".to_string(),
                "Bank Penyalur".to_string(), "Nomor Rekening".to_string(), "Rekening Atas Nama".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "gradebook-ledger".to_string(),
            title: "Ledger Buku Nilai Rapor Semester & Capaian".to_string(),
            category: "Akademik & Nilai".to_string(),
            description: "Ledger nilai rapor semester resmi dari snapshot kurikulum merdeka (Formatif TP, SAS, Nilai Akhir, Predikat, dan KKTP).".to_string(),
            icon: "BarChart2".to_string(),
            file_prefix: "Ledger_Buku_Nilai_Rapor".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "NISN".to_string(), "Nama Siswa".to_string(), "Rombel".to_string(), "Mata Pelajaran".to_string(),
                "Kode Mapel".to_string(), "Nilai Formatif TP".to_string(), "Nilai Sumatif SAS".to_string(),
                "Nilai Akhir Rapor".to_string(), "Predikat".to_string(), "Status KKTP / KKM".to_string(),
                "Capaian Tertinggi".to_string(), "Perlu Peningkatan".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "attendance-recap".to_string(),
            title: "Rekapitulasi Presensi & Kehadiran Siswa Real".to_string(),
            category: "Presensi & Karakter".to_string(),
            description: "Statistik kehadiran akurat dari PostgreSQL session_attendances (Hadir, Sakit, Izin, Alpa, dan % Persentase).".to_string(),
            icon: "Smartphone".to_string(),
            file_prefix: "Rekapitulasi_Presensi_Siswa".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "NISN".to_string(), "Nama Siswa".to_string(), "Rombel".to_string(), "Total Pertemuan".to_string(),
                "Hadir (H)".to_string(), "Sakit (S)".to_string(), "Izin (I)".to_string(), "Alpa (A)".to_string(),
                "Persentase Kehadiran (%)".to_string(), "Status Kehadiran".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "teachers-staff".to_string(),
            title: "Direktori Guru & Tenaga Kependidikan (GTK)".to_string(),
            category: "Kepegawaian & Audit".to_string(),
            description: "Direktori lengkap Pendidik dan Tenaga Kependidikan dengan NUPTK, NIP, Jenis PTK, Status Kepegawaian, dan Mapel Diampu.".to_string(),
            icon: "Globe".to_string(),
            file_prefix: "Direktori_GTK_Sekolah".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "NUPTK".to_string(), "NIP".to_string(), "Nama Lengkap & Gelar".to_string(),
                "Jenis Kelamin".to_string(), "Tempat Lahir".to_string(), "Tanggal Lahir".to_string(),
                "Jabatan PTK".to_string(), "Status Kepegawaian".to_string(), "Mata Pelajaran Diampu".to_string(),
                "No HP".to_string(), "Email".to_string(), "Status Keaktifan".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "cbt-exams".to_string(),
            title: "Laporan Ujian CBT & Analisis Ketuntasan KKM".to_string(),
            category: "Akademik & Nilai".to_string(),
            description: "Rekapitulasi hasil ujian CBT online siswa, nilai perolehan, perbandingan KKM, dan status kelulusan.".to_string(),
            icon: "BarChart2".to_string(),
            file_prefix: "Laporan_Ujian_CBT".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "Judul Ujian CBT".to_string(), "Mata Pelajaran".to_string(), "Rombel".to_string(),
                "NISN Siswa".to_string(), "Nama Siswa".to_string(), "Nilai Diperoleh".to_string(),
                "KKM Lulus".to_string(), "Status Kelulusan".to_string(), "Waktu Mulai".to_string(),
                "Waktu Selesai".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "schedules".to_string(),
            title: "Jadwal Pelajaran & Alokasi Ruang Kelas".to_string(),
            category: "Akademik & Nilai".to_string(),
            description: "Matriks jadwal mengajar mingguan guru, alokasi jam tatap muka rombel, dan pemetaan ruangan kelas.".to_string(),
            icon: "Folder".to_string(),
            file_prefix: "Jadwal_Pelajaran_Sekolah".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "Hari".to_string(), "Jam Mulai".to_string(), "Jam Selesai".to_string(),
                "Rombel / Kelas".to_string(), "Mata Pelajaran".to_string(), "Guru Pengampu".to_string(),
                "Ruang Kelas".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "habits-character".to_string(),
            title: "Monitoring 7 Pembiasaan Karakter (G7KAIH)".to_string(),
            category: "Presensi & Karakter".to_string(),
            description: "Rekapitulasi longitudinal pembiasaan anak Indonesia hebat (Bangun Pagi, Ibadah, Olahraga, dsb.) & verifikasi wali.".to_string(),
            icon: "GraduationCap".to_string(),
            file_prefix: "Monitoring_7_Pembiasaan_Karakter".to_string(),
            default_format: "XLSX".to_string(),
            columns: vec![
                "Tanggal".to_string(), "NISN".to_string(), "Nama Siswa".to_string(), "Rombel".to_string(),
                "Bangun Pagi".to_string(), "Beribadah".to_string(), "Berolahraga".to_string(), "Makan Sehat".to_string(),
                "Gemar Belajar".to_string(), "Bermasyarakat".to_string(), "Tidur Tepat Waktu".to_string(),
                "Total Terpenuhi (0-7)".to_string(), "Persentase (%)".to_string(), "Status Verifikasi".to_string(),
                "Catatan Orang Tua".to_string(),
            ],
        },
        ReportTypeInfo {
            id: "audit-security".to_string(),
            title: "Log Audit Keamanan Sistem & Aktivitas Pengguna".to_string(),
            category: "Kepegawaian & Audit".to_string(),
            description: "Rekam jejak komprehensif audit keamanan, mutasi data, otorisasi, dan aktivitas pengguna di PostgreSQL.".to_string(),
            icon: "ShieldCheck".to_string(),
            file_prefix: "Audit_Security_Logs".to_string(),
            default_format: "CSV".to_string(),
            columns: vec![
                "Timestamp (WIB)".to_string(), "Nama Pengguna".to_string(), "Email".to_string(),
                "Aksi Operasi".to_string(), "Entitas Resource".to_string(), "IP Address".to_string(),
                "User Agent".to_string(),
            ],
        },
    ];

    Ok(Json(ApiResponse::success(types, req_ctx.request_id)))
}

#[utoipa::path(
    get,
    operation_id = "getReportHubStats",
    path = "/api/v1/reports/stats",
    responses(
        (status = 200, description = "Statistics for Report Hub", body = ApiResponse<ReportHubStats>),
        (status = 401, description = "Unauthorized")
    ),
    security(("Bearer" = [])),
    tag = "Reports"
)]
pub async fn get_report_hub_stats(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<ReportHubStats>>, ApiError> {
    let pool = &ctx.pool;
    let tid = req_ctx.tenant_id;

    let (
        tot_stud,
        act_stud,
        mut_stud,
        pip_stud,
        tot_teach,
        tot_staff,
        tot_cls,
        tot_quiz,
        tot_sched,
        tot_hab,
        tot_audit,
    ) = tokio::join!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND (status IN ('Active', 'active') OR (status IS NULL AND is_active = true)) AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND (status IN ('transferred', 'Transferred', 'MUTATED', 'DROPPED_OUT', 'GRADUATED') OR is_active = false) AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM students WHERE tenant_id = $1 AND (penerima_kip = true OR layak_pip = true OR penerima_kps = true) AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM teachers WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM staff WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM classes WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM quizzes WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM class_schedules WHERE tenant_id = $1 AND deleted_at IS NULL").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM habit_tracker_entries WHERE tenant_id = $1").bind(tid).fetch_one(pool),
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_logs WHERE tenant_id = $1").bind(tid).fetch_one(pool),
    );

    let active_students = act_stud.unwrap_or(0);
    let stats = ReportHubStats {
        total_students: active_students,
        active_students,
        mutated_students: mut_stud.unwrap_or(0),
        pip_students: pip_stud.unwrap_or(0),
        total_teachers: tot_teach.unwrap_or(0),
        total_staff: tot_staff.unwrap_or(0),
        total_classes: tot_cls.unwrap_or(0),
        total_quizzes: tot_quiz.unwrap_or(0),
        total_schedules: tot_sched.unwrap_or(0),
        total_habits: tot_hab.unwrap_or(0),
        total_audit_logs: tot_audit.unwrap_or(0),
    };

    Ok(Json(ApiResponse::success(stats, req_ctx.request_id)))
}

fn escape_csv(val: &str) -> String {
    if val.contains(',') || val.contains('"') || val.contains('\n') || val.contains('\r') {
        format!("\"{}\"", val.replace('"', "\"\""))
    } else {
        val.to_string()
    }
}

pub async fn export_report(
    State(ctx): State<ApplicationContext>,
    Path(report_type): Path<String>,
    Query(query): Query<ReportExportQuery>,
    req_ctx: RequestContext,
) -> Result<Response, ApiError> {
    let pool = &ctx.pool;
    let tid = req_ctx.tenant_id;
    let format = query.format.as_deref().unwrap_or("json").to_lowercase();

    // Check authorization: must be admin or staff or teacher
    if let Some(ref actor) = req_ctx.actor {
        let has_export_perm = actor.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin")
                || n.contains("kepala")
                || n.contains("staff")
                || n.contains("tata usaha")
                || n.contains("guru")
        });
        if !has_export_perm {
            return Err(ApiError::new(
                ApplicationError::Forbidden(
                    ErrorCode::AuthPermissionDenied,
                    "Akses ditolak: Ekspor data resmi sekolah memerlukan hak akses Administrator, Tata Usaha, atau Pendidik.".to_string(),
                ),
                req_ctx.request_id,
            ));
        }
    }

    let (headers_list, rows_data, filename_prefix) = match report_type.as_str() {
        "master-dapodik" => {
            let sql = r#"
                SELECT 
                    COALESCE(s.nisn, '') as nisn,
                    COALESCE(s.nik, '') as nik,
                    COALESCE(s.nipd, '') as nipd,
                    s.full_name,
                    COALESCE(s.gender, '') as gender,
                    COALESCE(s.place_of_birth, '') as place_of_birth,
                    COALESCE(TO_CHAR(s.date_of_birth, 'YYYY-MM-DD'), '') as date_of_birth,
                    COALESCE(s.religion, '') as religion,
                    COALESCE(c.name, s.rombel, 'Belum Diplot') as rombel,
                    COALESCE(s.alamat_jalan, '') as alamat_jalan,
                    COALESCE(s.rt, '') as rt,
                    COALESCE(s.rw, '') as rw,
                    COALESCE(s.dusun, '') as dusun,
                    COALESCE(s.kelurahan, '') as kelurahan,
                    COALESCE(s.kecamatan, '') as kecamatan,
                    COALESCE(s.kode_pos, '') as kode_pos,
                    COALESCE(s.no_hp, '') as no_hp,
                    COALESCE(s.nama_ayah, '') as nama_ayah,
                    COALESCE(s.nama_ibu, '') as nama_ibu,
                    COALESCE(s.nama_wali, '') as nama_wali,
                    CASE 
                        WHEN s.status IN ('Active', 'active') OR (s.status IS NULL AND s.is_active = true) THEN 'Aktif'
                        WHEN s.status IN ('transferred', 'Transferred') THEN 'Mutasi / Keluar'
                        ELSE COALESCE(s.status, 'Aktif')
                    END as status
                FROM students s
                LEFT JOIN enrollments e ON e.student_id = s.id AND (e.status = 'Active' OR e.status = 'active')
                LEFT JOIN classes c ON c.id = e.class_id AND c.deleted_at IS NULL
                WHERE s.tenant_id = $1 AND s.deleted_at IS NULL
                  AND ($2::uuid IS NULL OR c.id = $2)
                ORDER BY c.name ASC NULLS LAST, s.full_name ASC
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .bind(query.class_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "NISN",
                "NIK",
                "NIPD",
                "Nama Lengkap Siswa",
                "Jenis Kelamin",
                "Tempat Lahir",
                "Tanggal Lahir",
                "Agama",
                "Rombel",
                "Alamat Jalan",
                "RT",
                "RW",
                "Dusun",
                "Kelurahan",
                "Kecamatan",
                "Kode Pos",
                "No HP",
                "Nama Ayah",
                "Nama Ibu",
                "Nama Wali",
                "Status Dapodik",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("nisn").unwrap_or_default(),
                    r.get::<Option<String>, _>("nik").unwrap_or_default(),
                    r.get::<Option<String>, _>("nipd").unwrap_or_default(),
                    r.get::<Option<String>, _>("full_name").unwrap_or_default(),
                    r.get::<Option<String>, _>("gender").unwrap_or_default(),
                    r.get::<Option<String>, _>("place_of_birth")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("date_of_birth")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("religion").unwrap_or_default(),
                    r.get::<Option<String>, _>("rombel").unwrap_or_default(),
                    r.get::<Option<String>, _>("alamat_jalan")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("rt").unwrap_or_default(),
                    r.get::<Option<String>, _>("rw").unwrap_or_default(),
                    r.get::<Option<String>, _>("dusun").unwrap_or_default(),
                    r.get::<Option<String>, _>("kelurahan").unwrap_or_default(),
                    r.get::<Option<String>, _>("kecamatan").unwrap_or_default(),
                    r.get::<Option<String>, _>("kode_pos").unwrap_or_default(),
                    r.get::<Option<String>, _>("no_hp").unwrap_or_default(),
                    r.get::<Option<String>, _>("nama_ayah").unwrap_or_default(),
                    r.get::<Option<String>, _>("nama_ibu").unwrap_or_default(),
                    r.get::<Option<String>, _>("nama_wali").unwrap_or_default(),
                    r.get::<Option<String>, _>("status").unwrap_or_default(),
                ]);
            }

            (cols, data, "Master_Siswa_Dapodik")
        }

        "mutasi-siswa" => {
            let sql = r#"
                SELECT 
                    COALESCE(s.nisn, '') as nisn,
                    s.full_name,
                    COALESCE(s.gender, '') as gender,
                    'Pindah / Mutasi Keluar' as jenis_mutasi,
                    COALESCE(TO_CHAR(s.updated_at, 'YYYY-MM-DD'), '') as tanggal_mutasi,
                    '-' as sekolah_tujuan,
                    '-' as nomor_sk,
                    COALESCE(s.rombel, 'Belum Diplot') as rombel_terakhir,
                    'Siswa pindah sekolah / mutasi' as keterangan
                FROM students s
                WHERE s.tenant_id = $1 
                  AND (s.status IN ('transferred', 'Transferred', 'MUTATED', 'DROPPED_OUT', 'GRADUATED') OR s.is_active = false)
                ORDER BY s.updated_at DESC
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "NISN",
                "Nama Siswa",
                "Jenis Kelamin",
                "Jenis Mutasi",
                "Tanggal Mutasi",
                "Sekolah Tujuan",
                "Nomor SK Mutasi",
                "Rombel Terakhir",
                "Alasan / Keterangan",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("nisn").unwrap_or_default(),
                    r.get::<Option<String>, _>("full_name").unwrap_or_default(),
                    r.get::<Option<String>, _>("gender").unwrap_or_default(),
                    r.get::<Option<String>, _>("jenis_mutasi")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("tanggal_mutasi")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("sekolah_tujuan")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("nomor_sk").unwrap_or_default(),
                    r.get::<Option<String>, _>("rombel_terakhir")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("keterangan").unwrap_or_default(),
                ]);
            }

            (cols, data, "Buku_Induk_Mutasi_Siswa")
        }

        "pip-bantuan" => {
            let sql = r#"
                SELECT 
                    COALESCE(s.nisn, '') as nisn,
                    s.full_name,
                    COALESCE(c.name, s.rombel, 'Belum Diplot') as rombel,
                    CASE WHEN s.penerima_kip = true THEN 'Ya' ELSE 'Tidak' END as penerima_kip,
                    COALESCE(s.nomor_kip, '-') as nomor_kip,
                    COALESCE(s.nama_di_kip, s.full_name) as nama_di_kip,
                    CASE WHEN s.penerima_kps = true THEN 'Ya' ELSE 'Tidak' END as penerima_kps,
                    COALESCE(s.no_kps, '-') as nomor_kps,
                    COALESCE(s.nomor_kks, '-') as nomor_kks,
                    CASE WHEN s.layak_pip = true THEN 'Ya' ELSE 'Tidak' END as layak_pip,
                    COALESCE(s.alasan_layak_pip, '-') as alasan_layak_pip,
                    COALESCE(s.bank, 'BRI') as bank_penyalur,
                    COALESCE(s.nomor_rekening_bank, '-') as nomor_rekening,
                    COALESCE(s.rekening_atas_nama, s.full_name) as rekening_atas_nama
                FROM students s
                LEFT JOIN enrollments e ON e.student_id = s.id AND (e.status = 'Active' OR e.status = 'active')
                LEFT JOIN classes c ON c.id = e.class_id AND c.deleted_at IS NULL
                WHERE s.tenant_id = $1 AND s.deleted_at IS NULL
                  AND (s.penerima_kip = true OR s.layak_pip = true OR s.penerima_kps = true OR s.nomor_kks IS NOT NULL)
                ORDER BY c.name ASC NULLS LAST, s.full_name ASC
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "NISN",
                "Nama Siswa",
                "Rombel",
                "Penerima KIP",
                "Nomor KIP",
                "Nama di KIP",
                "Penerima KPS",
                "Nomor KPS",
                "Nomor KKS",
                "Layak PIP",
                "Alasan Layak PIP",
                "Bank Penyalur",
                "Nomor Rekening",
                "Rekening Atas Nama",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("nisn").unwrap_or_default(),
                    r.get::<Option<String>, _>("full_name").unwrap_or_default(),
                    r.get::<Option<String>, _>("rombel").unwrap_or_default(),
                    r.get::<Option<String>, _>("penerima_kip")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("nomor_kip").unwrap_or_default(),
                    r.get::<Option<String>, _>("nama_di_kip")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("penerima_kps")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("nomor_kps").unwrap_or_default(),
                    r.get::<Option<String>, _>("nomor_kks").unwrap_or_default(),
                    r.get::<Option<String>, _>("layak_pip").unwrap_or_default(),
                    r.get::<Option<String>, _>("alasan_layak_pip")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("bank_penyalur")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("nomor_rekening")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("rekening_atas_nama")
                        .unwrap_or_default(),
                ]);
            }

            (cols, data, "Rekap_Bantuan_PIP_KIP")
        }

        "gradebook-ledger" => {
            let sql = r#"
                SELECT 
                    COALESCE(s.nisn, '') as nisn,
                    s.full_name,
                    COALESCE(c.name, s.rombel, 'Belum Diplot') as rombel,
                    COALESCE(sub.name, gs.subject_name, 'Mata Pelajaran') as mata_pelajaran,
                    COALESCE(sub.code, gs.subject_code, '-') as kode_mapel,
                    COALESCE(gs.avg_summative_tp::text, '0.0') as nilai_formatif_tp,
                    COALESCE(gs.sas_score::text, '0.0') as nilai_sumatif_sas,
                    COALESCE(gs.final_score::text, g.final_score::text, '0.0') as nilai_akhir_rapor,
                    COALESCE(g.letter_grade, CASE WHEN COALESCE(gs.final_score, g.final_score, 0.0) >= 88 THEN 'A' WHEN COALESCE(gs.final_score, g.final_score, 0.0) >= 75 THEN 'B' WHEN COALESCE(gs.final_score, g.final_score, 0.0) >= 60 THEN 'C' ELSE 'D' END) as predikat,
                    COALESCE(gs.status_kktp, CASE WHEN COALESCE(gs.final_score, g.final_score, 0.0) >= 75 THEN 'TUNTAS' ELSE 'PERLU_REMEDIAL' END) as status_kkm,
                    COALESCE(gs.highest_achievement_desc, '-') as capaian_tertinggi,
                    COALESCE(gs.lowest_improvement_desc, '-') as perlu_peningkatan
                FROM students s
                LEFT JOIN enrollments e ON e.student_id = s.id AND (e.status = 'Active' OR e.status = 'active')
                LEFT JOIN classes c ON c.id = e.class_id AND c.deleted_at IS NULL
                LEFT JOIN gradebook_snapshots gs ON gs.student_id = s.id AND gs.tenant_id = s.tenant_id
                LEFT JOIN gradebooks g ON g.student_id = s.id AND g.tenant_id = s.tenant_id
                LEFT JOIN subjects sub ON sub.id = g.subject_id
                WHERE s.tenant_id = $1 AND s.deleted_at IS NULL
                  AND ($2::uuid IS NULL OR c.id = $2)
                ORDER BY c.name ASC NULLS LAST, s.full_name ASC
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .bind(query.class_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "NISN",
                "Nama Siswa",
                "Rombel",
                "Mata Pelajaran",
                "Kode Mapel",
                "Nilai Formatif TP",
                "Nilai Sumatif SAS",
                "Nilai Akhir Rapor",
                "Predikat",
                "Status KKTP / KKM",
                "Capaian Tertinggi",
                "Perlu Peningkatan",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("nisn").unwrap_or_default(),
                    r.get::<Option<String>, _>("full_name").unwrap_or_default(),
                    r.get::<Option<String>, _>("rombel").unwrap_or_default(),
                    r.get::<Option<String>, _>("mata_pelajaran")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("kode_mapel").unwrap_or_default(),
                    r.get::<Option<String>, _>("nilai_formatif_tp")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("nilai_sumatif_sas")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("nilai_akhir_rapor")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("predikat").unwrap_or_default(),
                    r.get::<Option<String>, _>("status_kkm").unwrap_or_default(),
                    r.get::<Option<String>, _>("capaian_tertinggi")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("perlu_peningkatan")
                        .unwrap_or_default(),
                ]);
            }

            (cols, data, "Ledger_Buku_Nilai_Rapor")
        }

        "attendance-recap" => {
            let sql = r#"
                SELECT 
                    COALESCE(s.nisn, '') as nisn,
                    s.full_name,
                    COALESCE(c.name, s.rombel, 'Belum Diplot') as rombel,
                    COUNT(sa.id)::text as total_pertemuan,
                    COUNT(sa.id) FILTER (WHERE sa.status = 'present')::text as hadir,
                    COUNT(sa.id) FILTER (WHERE sa.status = 'sick')::text as sakit,
                    COUNT(sa.id) FILTER (WHERE sa.status = 'excused')::text as izin,
                    COUNT(sa.id) FILTER (WHERE sa.status = 'absent')::text as alpa,
                    CASE 
                        WHEN COUNT(sa.id) > 0 THEN 
                            ROUND((COUNT(sa.id) FILTER (WHERE sa.status = 'present')::numeric / COUNT(sa.id)::numeric) * 100.0, 1)::text
                        ELSE '100.0'
                    END as persentase_kehadiran,
                    CASE 
                        WHEN COUNT(sa.id) = 0 THEN 'Belum Ada Sesi'
                        WHEN (COUNT(sa.id) FILTER (WHERE sa.status = 'present')::numeric / NULLIF(COUNT(sa.id)::numeric, 0)) >= 0.90 THEN 'Sangat Baik'
                        WHEN (COUNT(sa.id) FILTER (WHERE sa.status = 'present')::numeric / NULLIF(COUNT(sa.id)::numeric, 0)) >= 0.75 THEN 'Baik'
                        ELSE 'Perlu Pembinaan'
                    END as status_kehadiran
                FROM students s
                LEFT JOIN enrollments e ON e.student_id = s.id AND (e.status = 'Active' OR e.status = 'active')
                LEFT JOIN classes c ON c.id = e.class_id AND c.deleted_at IS NULL
                LEFT JOIN session_attendances sa ON sa.student_id = s.id
                WHERE s.tenant_id = $1 AND s.deleted_at IS NULL
                  AND ($2::uuid IS NULL OR c.id = $2)
                GROUP BY s.id, s.nisn, s.full_name, c.name, s.rombel
                ORDER BY c.name ASC NULLS LAST, s.full_name ASC
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .bind(query.class_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "NISN",
                "Nama Siswa",
                "Rombel",
                "Total Pertemuan",
                "Hadir (H)",
                "Sakit (S)",
                "Izin (I)",
                "Alpa (A)",
                "Persentase Kehadiran (%)",
                "Status Kehadiran",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("nisn").unwrap_or_default(),
                    r.get::<Option<String>, _>("full_name").unwrap_or_default(),
                    r.get::<Option<String>, _>("rombel").unwrap_or_default(),
                    r.get::<Option<String>, _>("total_pertemuan")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("hadir").unwrap_or_default(),
                    r.get::<Option<String>, _>("sakit").unwrap_or_default(),
                    r.get::<Option<String>, _>("izin").unwrap_or_default(),
                    r.get::<Option<String>, _>("alpa").unwrap_or_default(),
                    r.get::<Option<String>, _>("persentase_kehadiran")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("status_kehadiran")
                        .unwrap_or_default(),
                ]);
            }

            (cols, data, "Rekapitulasi_Presensi_Siswa")
        }

        "teachers-staff" => {
            let sql = r#"
                SELECT 
                    COALESCE(t.nuptk, '-') as nuptk,
                    COALESCE(t.nip, '-') as nip,
                    t.full_name,
                    COALESCE(t.jk, '-') as jenis_kelamin,
                    COALESCE(t.tempat_lahir, '-') as tempat_lahir,
                    COALESCE(TO_CHAR(t.tanggal_lahir, 'YYYY-MM-DD'), '-') as tanggal_lahir,
                    COALESCE(t.jenis_ptk, 'Guru Mata Pelajaran') as jabatan_ptk,
                    COALESCE(t.status_kepegawaian, 'Pendidik Tetap') as status_kepegawaian,
                    COALESCE(t.subject, '-') as mata_pelajaran,
                    COALESCE(t.no_hp, '-') as no_hp,
                    COALESCE(t.email, '-') as email,
                    CASE WHEN t.is_active = true THEN 'Aktif Terdaftar' ELSE 'Non-Aktif' END as status_aktif
                FROM teachers t
                WHERE t.tenant_id = $1 AND t.deleted_at IS NULL
                UNION ALL
                SELECT 
                    COALESCE(st.nuptk, '-') as nuptk,
                    COALESCE(st.nip, '-') as nip,
                    st.full_name,
                    COALESCE(st.jk, '-') as jenis_kelamin,
                    COALESCE(st.tempat_lahir, '-') as tempat_lahir,
                    COALESCE(TO_CHAR(st.tanggal_lahir, 'YYYY-MM-DD'), '-') as tanggal_lahir,
                    COALESCE(st.jenis_ptk, 'Tenaga Kependidikan / TU') as jabatan_ptk,
                    COALESCE(st.status_kepegawaian, 'Staf Administrasi') as status_kepegawaian,
                    '-' as mata_pelajaran,
                    COALESCE(st.no_hp, '-') as no_hp,
                    COALESCE(st.email, '-') as email,
                    CASE WHEN st.is_active = true THEN 'Aktif Terdaftar' ELSE 'Non-Aktif' END as status_aktif
                FROM staff st
                WHERE st.tenant_id = $1 AND st.deleted_at IS NULL
                ORDER BY jabatan_ptk ASC, full_name ASC
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "NUPTK",
                "NIP",
                "Nama Lengkap & Gelar",
                "Jenis Kelamin",
                "Tempat Lahir",
                "Tanggal Lahir",
                "Jabatan PTK",
                "Status Kepegawaian",
                "Mata Pelajaran Diampu",
                "No HP",
                "Email",
                "Status Keaktifan",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("nuptk").unwrap_or_default(),
                    r.get::<Option<String>, _>("nip").unwrap_or_default(),
                    r.get::<Option<String>, _>("full_name").unwrap_or_default(),
                    r.get::<Option<String>, _>("jenis_kelamin")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("tempat_lahir")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("tanggal_lahir")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("jabatan_ptk")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("status_kepegawaian")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("mata_pelajaran")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("no_hp").unwrap_or_default(),
                    r.get::<Option<String>, _>("email").unwrap_or_default(),
                    r.get::<Option<String>, _>("status_aktif")
                        .unwrap_or_default(),
                ]);
            }

            (cols, data, "Direktori_GTK_Sekolah")
        }

        "cbt-exams" => {
            let sql = r#"
                SELECT 
                    q.title as judul_ujian,
                    COALESCE(sub.name, 'Umum') as mata_pelajaran,
                    COALESCE(c.name, '-') as rombel,
                    COALESCE(s.nisn, '-') as nisn_siswa,
                    COALESCE(s.full_name, 'Peserta Ujian') as nama_siswa,
                    COALESCE(qa.score::text, '0.0') as nilai_diperoleh,
                    COALESCE(q.passing_score::text, '75.0') as kkm_lulus,
                    CASE 
                        WHEN COALESCE(qa.score, 0.0) >= COALESCE(q.passing_score, 75.0) THEN 'Lulus KKM'
                        ELSE 'Remedial'
                    END as status_kelulusan,
                    COALESCE(TO_CHAR(qa.started_at, 'YYYY-MM-DD HH24:MI'), '-') as waktu_mulai,
                    COALESCE(TO_CHAR(qa.completed_at, 'YYYY-MM-DD HH24:MI'), 'Sedang Mengerjakan') as waktu_selesai
                FROM quizzes q
                LEFT JOIN subjects sub ON sub.id = q.subject_id
                LEFT JOIN classes c ON c.id = q.class_id
                LEFT JOIN quiz_attempts qa ON qa.quiz_id = q.id
                LEFT JOIN students s ON s.id = qa.student_id
                WHERE q.tenant_id = $1 AND q.deleted_at IS NULL
                  AND ($2::uuid IS NULL OR c.id = $2)
                ORDER BY q.created_at DESC, qa.score DESC NULLS LAST
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .bind(query.class_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "Judul Ujian CBT",
                "Mata Pelajaran",
                "Rombel",
                "NISN Siswa",
                "Nama Siswa",
                "Nilai Diperoleh",
                "KKM Lulus",
                "Status Kelulusan",
                "Waktu Mulai",
                "Waktu Selesai",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("judul_ujian")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("mata_pelajaran")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("rombel").unwrap_or_default(),
                    r.get::<Option<String>, _>("nisn_siswa").unwrap_or_default(),
                    r.get::<Option<String>, _>("nama_siswa").unwrap_or_default(),
                    r.get::<Option<String>, _>("nilai_diperoleh")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("kkm_lulus").unwrap_or_default(),
                    r.get::<Option<String>, _>("status_kelulusan")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("waktu_mulai")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("waktu_selesai")
                        .unwrap_or_default(),
                ]);
            }

            (cols, data, "Laporan_Ujian_CBT")
        }

        "schedules" => {
            let sql = r#"
                SELECT 
                    COALESCE(cs.day_of_week, 'Senin') as hari,
                    COALESCE(cs.start_time, '-') as jam_mulai,
                    COALESCE(cs.end_time, '-') as jam_selesai,
                    COALESCE(c.name, '-') as rombel_kelas,
                    COALESCE(sub.name, '-') as mata_pelajaran,
                    COALESCE(t.full_name, 'Guru Pengampu') as guru_pengampu,
                    COALESCE(cs.room, 'Ruang Kelas') as ruangan
                FROM class_schedules cs
                JOIN classes c ON c.id = cs.class_id
                JOIN subjects sub ON sub.id = cs.subject_id
                LEFT JOIN teachers t ON t.id = cs.teacher_id
                WHERE cs.tenant_id = $1 AND cs.deleted_at IS NULL
                  AND ($2::uuid IS NULL OR c.id = $2)
                ORDER BY cs.day_of_week ASC, cs.start_time ASC
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .bind(query.class_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "Hari",
                "Jam Mulai",
                "Jam Selesai",
                "Rombel / Kelas",
                "Mata Pelajaran",
                "Guru Pengampu",
                "Ruang Kelas",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("hari").unwrap_or_default(),
                    r.get::<Option<String>, _>("jam_mulai").unwrap_or_default(),
                    r.get::<Option<String>, _>("jam_selesai")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("rombel_kelas")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("mata_pelajaran")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("guru_pengampu")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("ruangan").unwrap_or_default(),
                ]);
            }

            (cols, data, "Jadwal_Pelajaran_Sekolah")
        }

        "habits-character" => {
            let sql = r#"
                SELECT 
                    TO_CHAR(h.entry_date, 'YYYY-MM-DD') as tanggal,
                    COALESCE(s.nisn, '-') as nisn,
                    s.full_name,
                    COALESCE(c.name, s.rombel, '-') as rombel,
                    CASE WHEN (h.habits->>'bangun_pagi')::boolean IS TRUE THEN 'Ya' ELSE 'Tidak' END as bangun_pagi,
                    CASE WHEN (h.habits->>'beribadah')::boolean IS TRUE THEN 'Ya' ELSE 'Tidak' END as beribadah,
                    CASE WHEN (h.habits->>'berolahraga')::boolean IS TRUE THEN 'Ya' ELSE 'Tidak' END as berolahraga,
                    CASE WHEN (h.habits->>'makan_sehat')::boolean IS TRUE THEN 'Ya' ELSE 'Tidak' END as makan_sehat,
                    CASE WHEN (h.habits->>'gemar_belajar')::boolean IS TRUE THEN 'Ya' ELSE 'Tidak' END as gemar_belajar,
                    CASE WHEN (h.habits->>'bermasyarakat')::boolean IS TRUE THEN 'Ya' ELSE 'Tidak' END as bermasyarakat,
                    CASE WHEN (h.habits->>'tidur_tepat_waktu')::boolean IS TRUE THEN 'Ya' ELSE 'Tidak' END as tidur_tepat_waktu,
                    h.completed_count::text as total_kebiasaan_terpenuhi,
                    h.compliance_rate::text as persentase_kepatuhan,
                    h.verification_status as status_verifikasi,
                    COALESCE(h.parent_feedback, '-') as catatan_orang_tua
                FROM habit_tracker_entries h
                JOIN students s ON s.id = h.student_id
                LEFT JOIN enrollments e ON e.student_id = s.id AND (e.status = 'Active' OR e.status = 'active')
                LEFT JOIN classes c ON c.id = e.class_id
                WHERE h.tenant_id = $1
                  AND ($2::uuid IS NULL OR c.id = $2)
                ORDER BY h.entry_date DESC, s.full_name ASC
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .bind(query.class_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "Tanggal",
                "NISN",
                "Nama Siswa",
                "Rombel",
                "Bangun Pagi",
                "Beribadah",
                "Berolahraga",
                "Makan Sehat",
                "Gemar Belajar",
                "Bermasyarakat",
                "Tidur Tepat Waktu",
                "Total Terpenuhi (0-7)",
                "Persentase (%)",
                "Status Verifikasi",
                "Catatan Orang Tua",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("tanggal").unwrap_or_default(),
                    r.get::<Option<String>, _>("nisn").unwrap_or_default(),
                    r.get::<Option<String>, _>("full_name").unwrap_or_default(),
                    r.get::<Option<String>, _>("rombel").unwrap_or_default(),
                    r.get::<Option<String>, _>("bangun_pagi")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("beribadah").unwrap_or_default(),
                    r.get::<Option<String>, _>("berolahraga")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("makan_sehat")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("gemar_belajar")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("bermasyarakat")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("tidur_tepat_waktu")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("total_kebiasaan_terpenuhi")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("persentase_kepatuhan")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("status_verifikasi")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("catatan_orang_tua")
                        .unwrap_or_default(),
                ]);
            }

            (cols, data, "Monitoring_7_Pembiasaan_Karakter")
        }

        "audit-security" => {
            let sql = r#"
                SELECT 
                    TO_CHAR(a.timestamp, 'YYYY-MM-DD HH24:MI:SS') as timestamp_wib,
                    COALESCE(u.full_name, 'Sistem') as nama_pengguna,
                    COALESCE(u.email, '-') as email,
                    a.action as aksi_operasi,
                    COALESCE(a.resource, '-') as entitas_resource,
                    COALESCE(a.ip, '-') as ip_address,
                    COALESCE(a.user_agent, '-') as user_agent
                FROM audit_logs a
                LEFT JOIN users u ON u.id = a.actor_id
                WHERE a.tenant_id = $1
                ORDER BY a.timestamp DESC
                LIMIT 1000
            "#;

            let db_rows = sqlx::query(sql)
                .bind(tid)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            let cols = vec![
                "Timestamp (WIB)",
                "Nama Pengguna",
                "Email",
                "Aksi Operasi",
                "Entitas Resource",
                "IP Address",
                "User Agent",
            ];

            let mut data: Vec<Vec<String>> = Vec::with_capacity(db_rows.len());
            for r in db_rows {
                data.push(vec![
                    r.get::<Option<String>, _>("timestamp_wib")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("nama_pengguna")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("email").unwrap_or_default(),
                    r.get::<Option<String>, _>("aksi_operasi")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("entitas_resource")
                        .unwrap_or_default(),
                    r.get::<Option<String>, _>("ip_address").unwrap_or_default(),
                    r.get::<Option<String>, _>("user_agent").unwrap_or_default(),
                ]);
            }

            (cols, data, "Audit_Security_Logs")
        }

        _ => {
            return Err(ApiError::new(
                ApplicationError::NotFound(
                    ErrorCode::ResourceNotFound,
                    format!(
                        "Laporan jenis '{}' tidak dikenali atau belum didukung.",
                        report_type
                    ),
                ),
                req_ctx.request_id,
            ));
        }
    };

    if format == "csv" {
        let mut csv = String::from("\u{FEFF}"); // UTF-8 BOM for Excel
        csv.push_str(
            &headers_list
                .iter()
                .map(|h| escape_csv(h))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');

        for row in rows_data {
            csv.push_str(
                &row.iter()
                    .map(|v| escape_csv(v))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            csv.push('\n');
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/csv; charset=utf-8"),
        );
        let cd_val = format!(
            "attachment; filename=\"{}_{}.csv\"",
            filename_prefix,
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        if let Ok(hv) = HeaderValue::from_str(&cd_val) {
            headers.insert(header::CONTENT_DISPOSITION, hv);
        }

        Ok((StatusCode::OK, headers, csv).into_response())
    } else {
        // Return JSON structured array with columns and rows mapped to objects for Excel/frontend client
        let mut json_rows = Vec::with_capacity(rows_data.len());
        for row in rows_data {
            let mut map = serde_json::Map::new();
            for (idx, val) in row.into_iter().enumerate() {
                if let Some(col_name) = headers_list.get(idx) {
                    map.insert(col_name.to_string(), serde_json::Value::String(val));
                }
            }
            json_rows.push(serde_json::Value::Object(map));
        }

        #[derive(Serialize)]
        struct ExportJsonResponse {
            pub report_type: String,
            pub filename_prefix: String,
            pub total_records: usize,
            pub columns: Vec<&'static str>,
            pub records: Vec<serde_json::Value>,
        }

        let payload = ExportJsonResponse {
            report_type,
            filename_prefix: filename_prefix.to_string(),
            total_records: json_rows.len(),
            columns: headers_list,
            records: json_rows,
        };

        Ok(Json(ApiResponse::success(payload, req_ctx.request_id)).into_response())
    }
}
