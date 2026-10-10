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

