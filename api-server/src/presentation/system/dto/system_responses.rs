use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TenantSummaryResponse {
    pub tenant_id: Uuid,
    pub tenant_name: String,
    pub school_name: Option<String>,
    pub npsn: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub server_status: String,
    pub student_count: i64,
    pub teacher_count: i64,
    pub class_count: i64,
    pub is_dapodik_connected: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ActivateMasterResponse {
    pub user_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub assigned_role: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SystemOverviewResponse {
    pub total_tenants: i64,
    pub active_tenants: i64,
    pub total_students: i64,
    pub total_teachers: i64,
    pub total_classes: i64,
    pub total_guardians: i64,
    pub outbox_pending_events: i64,
    pub server_engine: String,
    pub rust_version: String,
    pub database_status: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SystemAuditLogResponse {
    pub id: Uuid,
    pub tenant_name: String,
    pub event_type: String,
    pub details: String,
    pub created_at: DateTime<Utc>,
}

