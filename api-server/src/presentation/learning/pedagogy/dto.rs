use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CpSearchQuery {
    pub phase: Option<String>,
    pub subject: Option<String>, // subject code or name
    pub verification: Option<String>, // "ALL", "VERIFIED_ONLY", "DRAFT_ONLY"
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema, Clone)]
pub struct LearningOutcomeElementDto {
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub subject_code: Option<String>,
    pub subject_name: String,
    pub phase: String,
    pub target_grades: String,
    pub element_name: String,
    pub element_code: Option<String>,
    pub description: String,
    pub source_origin: String,
    pub source_version: String,
    pub source_document: Option<String>,
    pub document_page_ref: Option<String>,
    pub verification_status: String,
    pub is_eligible_source: bool,
    pub order_index: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, utoipa::ToSchema, Clone)]
pub struct CpRegistryLookupResponse {
    pub phase: String,
    pub subject_query: String,
    pub source_available: bool,
    pub eligible_for_ai_synthesis: bool,
    pub elements_count: usize,
    pub elements: Vec<LearningOutcomeElementDto>,
    pub message: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RegisterSchoolCpPayload {
    pub subject_name: String,
    pub subject_code: Option<String>,
    pub phase: String, // e.g. "FASE_A".."FASE_F"
    pub target_grades: String, // e.g. "Kelas 5-6 SD"
    pub element_name: String,
    pub element_code: Option<String>,
    pub description: String,
    pub source_document: Option<String>,
    pub document_page_ref: Option<String>,
    pub order_index: Option<i32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SynthesizeCpRequest {
    pub source_cp_id: Uuid,
    pub grade_level: Option<String>,
    pub academic_year: Option<String>,
    pub force_regenerate: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct ProposedTpDto {
    pub id: Uuid,
    pub code: String,
    pub competency: String,
    pub bloom_level: String,
    pub content_scope: String,
    pub statement: String,
    pub pancasila_profiles: Vec<String>,
    pub evidence_indicators: Vec<String>,
    pub estimated_hours: i32,
    pub publication_status: String,
    pub version: i32,
    pub semester: String,
    pub sequence_order: i32,
}

#[derive(Debug, Serialize, utoipa::ToSchema, Clone)]
pub struct SynthesizeCpResponse {
    pub cache_hit: bool,
    pub source_cp_id: Uuid,
    pub source_cp_element: String,
    pub source_verification_status: String,
    pub grade_level: String,
    pub academic_year: String,
    pub version: i32,
    pub proposed_tps_count: usize,
    pub proposed_tps: Vec<ProposedTpDto>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct KaldikBudgetQuery {
    pub subject_code: Option<String>,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema, Clone)]
pub struct KaldikBudgetResponse {
    pub academic_year: String,
    pub semester: String,
    pub meb_weeks: i32,
    pub heb_days: i32,
    pub weekly_hours: i32,
    pub total_capacity_jp: i32,
    pub allocated_jp: i32,
    pub remaining_available_jp: i32,
    pub allocation_percentage: f64,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct ModulAjarDto {
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub learning_objective_id: Uuid,
    pub tp_code: Option<String>,
    pub tp_statement: Option<String>,
    pub tp_publication_status: Option<String>,
    pub academic_year: String,
    pub semester: String,
    pub title: String,
    pub grade_level: String,
    pub subject_code: String,
    pub subject_name: String,
    pub phase: String,
    pub allocated_hours: i32,
    pub total_meetings: i32,
    pub hours_per_meeting: i32,
    pub pancasila_profiles: Vec<String>,
    pub meaningful_understanding: String,
    pub trigger_questions: serde_json::Value,
    pub differentiation_strategies: serde_json::Value,
    pub learning_activities: serde_json::Value,
    pub assessment_plan: serde_json::Value,
    pub lkpd_attachments: serde_json::Value,
    pub status: String,
    pub suspension_reason: Option<String>,
    pub version: i32,
    pub is_ai_generated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SynthesizeModulAjarPayload {
    pub learning_objective_id: Uuid,
    pub academic_year: Option<String>,
    pub semester: String,
    pub grade_level: String,
    pub subject_name: String,
    pub subject_code: String,
    pub allocated_hours: i32,
    pub total_meetings: i32,
    pub hours_per_meeting: i32,
    pub user_instructions: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateModulAjarPayload {
    pub title: Option<String>,
    pub meaningful_understanding: Option<String>,
    pub trigger_questions: Option<serde_json::Value>,
    pub differentiation_strategies: Option<serde_json::Value>,
    pub learning_activities: Option<serde_json::Value>,
    pub assessment_plan: Option<serde_json::Value>,
    pub lkpd_attachments: Option<serde_json::Value>,
    pub allocated_hours: Option<i32>,
    pub total_meetings: Option<i32>,
    pub hours_per_meeting: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct KospCommitteeDto {
    pub id: Uuid,
    pub teacher_id: Option<Uuid>,
    pub name: String,
    pub role_in_committee: String,
    pub order_index: i32,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct KospSignatoryDto {
    pub id: Uuid,
    pub signatory_role: String,
    pub position_title: String,
    pub name: String,
    pub nip: Option<String>,
    pub has_signature: bool,
    pub has_official_seal: bool,
    pub order_index: i32,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct KospDocumentResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub academic_year: String,
    pub title: String,
    pub legal_basis_statement: String,
    pub establishment_place: String,
    pub establishment_date: String,
    pub effective_weeks_odd: i32,
    pub effective_weeks_even: i32,
    pub status: String,
    pub committees: Vec<KospCommitteeDto>,
    pub signatories: Vec<KospSignatoryDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ─────────────────────────────────────────────────────────────────────────────
// ATP (Alur Tujuan Pembelajaran) DTOs
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct AtpMatrixQuery {
    pub source_cp_id: Uuid,
    pub academic_year: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct AtpItemDto {
    pub tp_id: Uuid,
    pub code: String,
    pub competency: Option<String>,
    pub bloom_level: Option<String>,
    pub content_scope: String,
    pub statement: String,
    pub pancasila_profiles: Vec<String>,
    pub evidence_indicators: Vec<String>,
    pub estimated_hours: i32,
    pub publication_status: String,
    pub version: i32,
    pub atp_id: Option<Uuid>,
    pub semester: Option<String>,
    pub sequence_order: Option<i32>,
    pub allocated_hours: Option<i32>,
    pub pedagogical_approach: Option<String>,
    pub subject_name: String,
    pub phase: String,
    pub element_name: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct AtpSemesterSummaryDto {
    pub items_count: usize,
    pub total_hours: i32,
    pub items: Vec<AtpItemDto>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct AtpMatrixResponse {
    pub academic_year: String,
    pub total_items: usize,
    pub total_hours: i32,
    pub odd_semester: AtpSemesterSummaryDto,
    pub even_semester: AtpSemesterSummaryDto,
}

// ─────────────────────────────────────────────────────────────────────────────
// TP (Tujuan Pembelajaran) Update DTO
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateTpPayload {
    pub id: Uuid,
    pub statement: Option<String>,
    pub competency: Option<String>,
    pub bloom_level: Option<String>,
    pub content_scope: Option<String>,
    pub estimated_hours: Option<i32>,
    pub pancasila_profiles: Option<Vec<String>>,
    pub evidence_indicators: Option<Vec<String>>,
    pub mark_as_reviewed: Option<bool>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Publish Pedagogy Transaction DTO
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PublishPedagogyPayload {
    pub tp_ids: Vec<Uuid>,
    pub academic_year: Option<String>,
    pub grade_level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct PublishPedagogyResponse {
    pub published_tps_count: usize,
    pub published_atps_count: usize,
    pub total_hours: i32,
    pub message: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Modul Ajar Query & Creation DTOs
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct ModulAjarQuery {
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub subject_code: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateModulAjarPayload {
    pub learning_objective_id: Uuid,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub title: String,
    pub grade_level: Option<String>,
    pub subject_code: Option<String>,
    pub subject_name: Option<String>,
    pub phase: Option<String>,
    pub allocated_hours: Option<i32>,
    pub total_meetings: Option<i32>,
    pub hours_per_meeting: Option<i32>,
    pub pancasila_profiles: Option<Vec<String>>,
    pub meaningful_understanding: Option<String>,
    pub trigger_questions: Option<serde_json::Value>,
    pub differentiation_strategies: Option<serde_json::Value>,
    pub learning_activities: Option<serde_json::Value>,
    pub assessment_plan: Option<serde_json::Value>,
    pub lkpd_attachments: Option<serde_json::Value>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pedagogical Assessments DTOs
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct AssessmentQuery {
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub class_id: Option<Uuid>,
    pub subject_code: Option<String>,
    pub taxonomy_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct PedagogicalAssessmentItemDto {
    pub id: Uuid,
    pub class_id: Uuid,
    pub class_name: Option<String>,
    pub learning_objective_id: Option<Uuid>,
    pub tp_code: Option<String>,
    pub tp_statement: Option<String>,
    pub tp_competency: Option<String>,
    pub tp_content_scope: Option<String>,
    pub tp_publication_status: Option<String>,
    pub modul_ajar_id: Option<Uuid>,
    pub modul_ajar_title: Option<String>,
    pub academic_year: String,
    pub semester: String,
    pub subject_code: String,
    pub subject_name: String,
    pub title: String,
    pub taxonomy_type: String,
    pub assessment_method: String,
    pub passing_threshold: f64,
    pub kktp_criteria: serde_json::Value,
    pub rubric_descriptors: serde_json::Value,
    pub date_conducted: String,
    pub calendar_event_id: Option<Uuid>,
    pub kaldik_event_title: Option<String>,
    pub status: String,
    pub graded_students_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateAssessmentPayload {
    pub learning_objective_id: Option<Uuid>,
    pub modul_ajar_id: Option<Uuid>,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub class_id: Uuid,
    pub subject_code: Option<String>,
    pub subject_name: Option<String>,
    pub title: String,
    pub taxonomy_type: String,
    pub assessment_method: Option<String>,
    pub passing_threshold: Option<f64>,
    pub kktp_criteria: Option<serde_json::Value>,
    pub rubric_descriptors: Option<serde_json::Value>,
    pub date_conducted: Option<String>,
    pub calendar_event_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct StudentGradeSubmission {
    pub student_id: Uuid,
    pub raw_score: Option<f64>,
    pub qualitative_level: Option<String>,
    pub feedback_notes: Option<String>,
    pub strengths: Option<String>,
    pub areas_for_improvement: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SubmitGradesPayload {
    pub grades: Vec<StudentGradeSubmission>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Habit Tracker G7KAIH DTOs
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct HabitsCalendarQuery {
    pub student_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
    pub entry_date: Option<String>,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub school_overview: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct HabitEntryDto {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: Option<String>,
    pub class_name: Option<String>,
    pub entry_date: String,
    pub academic_year: String,
    pub semester: String,
    pub habits: serde_json::Value,
    pub habits_notes: serde_json::Value,
    pub completed_count: i16,
    pub compliance_rate: f64,
    pub verification_status: String,
    pub verified_by_user_id: Option<Uuid>,
    pub verified_by_role: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub parent_feedback: Option<String>,
    pub teacher_notes: Option<String>,
    pub override_reason: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RecordHabitEntryPayload {
    pub student_id: Uuid,
    pub entry_date: String,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub habits: serde_json::Value,
    pub habits_notes: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct VerifyHabitParentPayload {
    pub entry_id: Uuid,
    pub parent_feedback: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct OverrideHabitTeacherPayload {
    pub entry_id: Uuid,
    pub teacher_notes: Option<String>,
    pub override_reason: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SuperviseModulAjarRequest {
    pub status: Option<String>,
    pub notes: Option<String>,
    pub supervisor_id: Option<Uuid>,
    pub supervisor_name: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct KaldikBudgetQuery {
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub subject_code: Option<String>,
    pub subject_name: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct KaldikBudgetDetails {
    pub academic_year: String,
    pub semester: String,
    pub meb_weeks: i32,
    pub heb_days: i32,
    pub weekly_hours: i32,
    pub total_capacity_jp: i32,
    pub allocated_jp: i32,
    pub remaining_available_jp: i32,
    pub allocation_percentage: i32,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SynthesizeModulAjarPayload {
    pub tp_id: Option<Uuid>,
    pub learning_objective_id: Option<Uuid>,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub grade_level: Option<String>,
    pub subject_name: Option<String>,
    pub subject_code: Option<String>,
    pub phase: Option<String>,
    pub allocated_hours: Option<i32>,
    pub total_meetings: Option<i32>,
    pub hours_per_meeting: Option<i32>,
    pub user_instructions: Option<String>,
    pub target_meetings: Option<i32>,
    pub pedagogical_approach: Option<String>,
    pub local_wisdom_context: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct DiagnosticClusteringQuery {
    pub class_id: Uuid,
    pub academic_year: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct RaporPreviewQuery {
    pub class_id: Uuid,
    pub subject_code: Option<String>,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ApproveRaporPayload {
    pub class_id: Uuid,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub subject_code: Option<String>,
    pub approved_by_role: Option<String>,
    pub approved_by_name: Option<String>,
    pub ratification_notes: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SemesterNarrativeQuery {
    pub class_id: Option<Uuid>,
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub student_id: Option<Uuid>,
    pub save_to_snapshot: Option<bool>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SendHabitReminderPayload {
    pub class_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    pub days_lookback: Option<i32>,
}
