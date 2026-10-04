use axum::{
    extract::{Path, Query, State},
    routing::{post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateScheduleRequest {
    pub class_id: Option<Uuid>,
    pub class_ids: Option<Vec<Uuid>>,
    pub subject_id: Uuid,
    pub teacher_id: Uuid,
    pub academic_year_id: Option<Uuid>,
    pub day_of_week: String,
    pub start_time: String,
    pub end_time: String,
    pub room: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateScheduleRequest {
    pub class_id: Option<Uuid>,
    pub subject_id: Option<Uuid>,
    pub teacher_id: Option<Uuid>,
    pub academic_year_id: Option<Uuid>,
    pub day_of_week: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
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
        .route("/{id}", put(update_schedule).delete(delete_schedule))
}

#[utoipa::path(
    post,
    operation_id = "createSchedule",
    path = "/api/v1/academic/schedules",
    request_body = CreateScheduleRequest,
    responses(
        (status = 201, description = "Schedules created", body = ApiResponse<Vec<ScheduleResponse>>)
    ),
    security(("Bearer" = []))
)]
async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateScheduleRequest>,
) -> Result<Json<ApiResponse<Vec<ScheduleResponse>>>, ApiError> {
    let target_class_ids = if let Some(ids) = payload.class_ids.filter(|v| !v.is_empty()) {
        ids
    } else if let Some(single_id) = payload.class_id {
        vec![single_id]
    } else {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "class_id atau class_ids wajib dipilih".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    };

    let room = payload.room.unwrap_or_else(|| "Ruang Kelas".to_string());
    let mut created_ids = Vec::new();

    for cid in &target_class_ids {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO class_schedules (
                id, tenant_id, class_id, subject_id, teacher_id, academic_year_id,
                day_of_week, start_time, end_time, room, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
            "#,
        )
        .bind(id)
        .bind(req_ctx.tenant_id)
        .bind(cid)
        .bind(payload.subject_id)
        .bind(payload.teacher_id)
        .bind(payload.academic_year_id)
        .bind(&payload.day_of_week)
        .bind(&payload.start_time)
        .bind(&payload.end_time)
        .bind(&room)
        .execute(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

        created_ids.push(id);
    }

    let rows = sqlx::query_as::<_, ScheduleResponse>(
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
            COALESCE(cs.room, 'Ruang Kelas') as room,
            cs.created_at
        FROM class_schedules cs
        JOIN classes c ON c.id = cs.class_id
        JOIN subjects s ON s.id = cs.subject_id
        JOIN teachers t ON t.id = cs.teacher_id
        WHERE cs.id = ANY($1) AND cs.tenant_id = $2
        ORDER BY c.name ASC
        "#,
    )
    .bind(&created_ids)
    .bind(req_ctx.tenant_id)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(rows, req_ctx.request_id)))
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
    let is_admin_or_staff = req_ctx
        .actor
        .as_ref()
        .map(|a| {
            a.roles.iter().any(|r| {
                r.name == "Kepala Sekolah"
                    || r.name == "Operator/Staff"
                    || r.name == "Admin"
                    || r.name == "SuperAdmin"
                    || r.name == "Bendahara"
            })
        })
        .unwrap_or(false);

    let is_teacher = !is_admin_or_staff
        && req_ctx
            .actor
            .as_ref()
            .map(|a| a.roles.iter().any(|r| r.name == "Guru" || r.name == "Teacher"))
            .unwrap_or(false);

    let mut filter_teacher_id = query.teacher_id;
    if is_teacher && filter_teacher_id.is_none() {
        let user_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
        let t_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM teachers 
            WHERE (
                user_id = $1 
                OR lower(trim(full_name)) = (SELECT lower(trim(full_name)) FROM users WHERE id = $1)
                OR (email IS NOT NULL AND lower(trim(email)) = (SELECT lower(trim(email)) FROM users WHERE id = $1))
            )
            AND tenant_id = $2 AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(req_ctx.tenant_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

        filter_teacher_id = t_id;
    }

    let rows = sqlx::query_as::<_, ScheduleResponse>(
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
            COALESCE(cs.room, 'Ruang Kelas') as room,
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
    )
    .bind(req_ctx.tenant_id)
    .bind(query.class_id)
    .bind(filter_teacher_id)
    .bind(query.day)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

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
    sqlx::query(
        "UPDATE class_schedules SET deleted_at = NOW() WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(true, req_ctx.request_id)))
}

#[utoipa::path(
    put,
    operation_id = "updateSchedule",
    path = "/api/v1/academic/schedules/{id}",
    request_body = UpdateScheduleRequest,
    responses(
        (status = 200, description = "Schedule updated", body = ApiResponse<ScheduleResponse>)
    ),
    security(("Bearer" = []))
)]
async fn update_schedule(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateScheduleRequest>,
) -> Result<Json<ApiResponse<ScheduleResponse>>, ApiError> {
    sqlx::query(
        r#"
        UPDATE class_schedules SET
            class_id = COALESCE($1, class_id),
            subject_id = COALESCE($2, subject_id),
            teacher_id = COALESCE($3, teacher_id),
            academic_year_id = COALESCE($4, academic_year_id),
            day_of_week = COALESCE($5, day_of_week),
            start_time = COALESCE($6, start_time),
            end_time = COALESCE($7, end_time),
            room = COALESCE($8, room),
            updated_at = NOW()
        WHERE id = $9 AND tenant_id = $10 AND deleted_at IS NULL
        "#
    )
    .bind(payload.class_id)
    .bind(payload.subject_id)
    .bind(payload.teacher_id)
    .bind(payload.academic_year_id)
    .bind(payload.day_of_week)
    .bind(payload.start_time)
    .bind(payload.end_time)
    .bind(payload.room)
    .bind(id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    let row = sqlx::query_as::<_, ScheduleResponse>(
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
            COALESCE(cs.room, 'Ruang Kelas') as room,
            cs.created_at
        FROM class_schedules cs
        JOIN classes c ON c.id = cs.class_id
        JOIN subjects s ON s.id = cs.subject_id
        JOIN teachers t ON t.id = cs.teacher_id
        WHERE cs.id = $1 AND cs.tenant_id = $2
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(row, req_ctx.request_id)))
}
