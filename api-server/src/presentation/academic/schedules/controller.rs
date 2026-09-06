use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{delete, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateScheduleRequest {
    pub class_id: Uuid,
    pub subject_id: Uuid,
    pub teacher_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub day_of_week: String,
    pub start_time: String,
    pub end_time: String,
    pub room: Option<String>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ScheduleResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub class_id: Uuid,
    pub class_name: String,
    pub subject_id: Uuid,
    pub subject_name: String,
    pub teacher_id: Uuid,
    pub teacher_name: String,
    pub teacher_user_id: Option<Uuid>,
    pub day_of_week: String,
    pub start_time: String,
    pub end_time: String,
    pub room: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct ScheduleFilterQuery {
    pub class_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub day: Option<String>,
}

pub fn schedule_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", delete(delete_schedule))
}

#[utoipa::path(
    post,
    operation_id = "createSchedule",
    path = "/api/v1/academic/schedules",
    request_body = CreateScheduleRequest,
    responses(
        (status = 201, description = "Schedule created", body = ApiResponse<ScheduleResponse>)
    ),
    security(("Bearer" = []))
)]
async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateScheduleRequest>,
) -> Result<Json<ApiResponse<ScheduleResponse>>, ApiError> {
    let room = payload.room.unwrap_or_else(|| "Ruang Kelas".to_string());
    let id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO class_schedules (
            id, tenant_id, class_id, subject_id, teacher_id, academic_year_id,
            day_of_week, start_time, end_time, room, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
        "#,
        id,
        req_ctx.tenant_id,
        payload.class_id,
        payload.subject_id,
        payload.teacher_id,
        payload.academic_year_id,
        payload.day_of_week,
        payload.start_time,
        payload.end_time,
        room
    )
    .execute(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let row = sqlx::query_as!(
        ScheduleResponse,
        r#"
        SELECT 
            cs.id,
            cs.tenant_id,
            cs.class_id,
            c.name as class_name,
            cs.subject_id,
            s.name as subject_name,
            cs.teacher_id,
            t.full_name as teacher_name,
            t.user_id as teacher_user_id,
            cs.day_of_week,
            cs.start_time,
            cs.end_time,
            COALESCE(cs.room, 'Ruang Kelas') as "room!",
            cs.created_at
        FROM class_schedules cs
        JOIN classes c ON c.id = cs.class_id
        JOIN subjects s ON s.id = cs.subject_id
        JOIN teachers t ON t.id = cs.teacher_id
        WHERE cs.id = $1 AND cs.tenant_id = $2
        "#,
        id,
        req_ctx.tenant_id
    )
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(row, req_ctx.request_id)))
}

#[utoipa::path(
    get,
    operation_id = "listSchedules",
    path = "/api/v1/academic/schedules",
    responses(
        (status = 200, description = "List of schedules", body = ApiResponse<Vec<ScheduleResponse>>)
    ),
    security(("Bearer" = []))
)]
async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<ScheduleFilterQuery>,
) -> Result<Json<ApiResponse<Vec<ScheduleResponse>>>, ApiError> {
    let rows = sqlx::query_as!(
        ScheduleResponse,
        r#"
        SELECT 
            cs.id,
            cs.tenant_id,
            cs.class_id,
            c.name as class_name,
            cs.subject_id,
            s.name as subject_name,
            cs.teacher_id,
            t.full_name as teacher_name,
            t.user_id as teacher_user_id,
            cs.day_of_week,
            cs.start_time,
            cs.end_time,
            COALESCE(cs.room, 'Ruang Kelas') as "room!",
            cs.created_at
        FROM class_schedules cs
        JOIN classes c ON c.id = cs.class_id
        JOIN subjects s ON s.id = cs.subject_id
        JOIN teachers t ON t.id = cs.teacher_id
        WHERE cs.tenant_id = $1 
          AND cs.deleted_at IS NULL
          AND ($2::uuid IS NULL OR cs.class_id = $2)
          AND ($3::uuid IS NULL OR cs.teacher_id = $3 OR t.user_id = $3)
          AND ($4::text IS NULL OR cs.day_of_week = $4)
        ORDER BY 
            CASE cs.day_of_week
                WHEN 'Senin' THEN 1
                WHEN 'Selasa' THEN 2
                WHEN 'Rabu' THEN 3
                WHEN 'Kamis' THEN 4
                WHEN 'Jumat' THEN 5
                WHEN 'Sabtu' THEN 6
                ELSE 7
            END,
            cs.start_time ASC
        "#,
        req_ctx.tenant_id,
        query.class_id,
        query.teacher_id,
        query.day
    )
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(rows, req_ctx.request_id)))
}

#[utoipa::path(
    delete,
    operation_id = "deleteSchedule",
    path = "/api/v1/academic/schedules/{id}",
    responses(
        (status = 200, description = "Schedule deleted", body = ApiResponse<bool>)
    ),
    security(("Bearer" = []))
)]
async fn delete_schedule(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<bool>>, ApiError> {
    sqlx::query!(
        "UPDATE class_schedules SET deleted_at = NOW() WHERE id = $1 AND tenant_id = $2",
        id,
        req_ctx.tenant_id
    )
    .execute(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(true, req_ctx.request_id)))
}
