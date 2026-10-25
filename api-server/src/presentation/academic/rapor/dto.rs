use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct SubjectCompetencyDto {
    pub subject_name: String,
    pub final_score: f64,
    pub predicate: String,
    pub description: String,
    pub group: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct AttendanceSummaryDto {
    pub sakit: i64,
    pub izin: i64,
    pub alpha: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ExtracurricularDto {
    pub name: String,
    pub predicate: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct StudentReportCardDto {
    pub student_id: Uuid,
    pub nisn: String,
    pub nipd: String,
    pub student_name: String,
    pub gender: String,
    pub birth_place_date: String,
    pub religion: String,
    pub address: String,
    pub father_name: String,
    pub mother_name: String,
    pub parent_address: String,
    pub parent_phone: String,
    pub class_name: String,
    pub phase: String,
    pub academic_year: String,
    pub semester: String,
    pub headmaster_name: String,
    pub headmaster_nip: String,
    pub teacher_name: String,
    pub guardian_name: String,
    pub subjects: Vec<SubjectCompetencyDto>,
    pub attendance: AttendanceSummaryDto,
    pub extracurricular: Vec<ExtracurricularDto>,
    pub p5_character_note: String,
    pub promotion_status: String,
    pub is_promotion_passed: bool,
    pub promotion_reason: Option<String>,
    pub qr_token: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ReportCardsResponse {
    pub academic_year: String,
    pub semester: i16,
    pub semester_label: String,
    pub school_name: String,
    pub npsn: String,
    pub headmaster_name: String,
    pub headmaster_nip: String,
    pub report_date: Option<String>,
    pub total_students: usize,
    pub cards: Vec<StudentReportCardDto>,
}
