use super::student::StudentStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Student List Read Models ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentSummary {
    pub id: Uuid,
    pub nisn: String,
    pub full_name: String,
    pub nik: Option<String>,
    pub gender: Option<String>,
    pub place_of_birth: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub religion: Option<String>,
    pub nipd: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub status: StudentStatus,
    pub class_name: Option<String>,
    pub grade: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentDetail {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub guardian_id: Option<Uuid>,
    pub nisn: String,
    pub full_name: String,
    pub nik: Option<String>,
    pub gender: Option<String>,
    pub place_of_birth: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub religion: Option<String>,
    pub nipd: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub status: StudentStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentProfile {
    pub student: StudentDetail,
    pub guardian: Option<GuardianSummary>,
    pub current_class: Option<ClassSummary>,
    pub current_enrollment: Option<EnrollmentSummary>,
    pub academic_year: Option<AcademicYearSummary>,
    pub attendance_summary: Option<AttendanceSummary>,
    pub latest_assessment_summary: Option<AssessmentSummary>,
}

// ─── Teacher Read Models ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherSummary {
    pub id: Uuid,
    pub nip: Option<String>,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub subject: Option<String>,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeacherDetail {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub nip: Option<String>,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub subject: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// ─── Guardian Read Models ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianDetail {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub full_name: String,
    pub phone_number: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// ─── Staff Read Models ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffSummary {
    pub id: Uuid,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub nip: Option<String>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub job_title: String,
    pub is_active: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffDetail {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub nip: Option<String>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub job_title: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

// ─── Related Read Model Stubs ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianSummary {
    pub id: Uuid,
    pub full_name: String,
    pub phone_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassSummary {
    pub id: Uuid,
    pub name: String,
    pub grade_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentSummary {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub enrolled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcademicYearSummary {
    pub id: Uuid,
    pub name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceSummary {
    pub total_present: u32,
    pub total_absent: u32,
    pub total_sick: u32,
    pub total_permitted: u32,
    pub attendance_rate_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentSummary {
    pub subject_name: String,
    pub average_score: f32,
    pub last_assessed_at: DateTime<Utc>,
}
