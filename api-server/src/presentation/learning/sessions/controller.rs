use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    attendance_response::AttendanceResponse, record_attendance_request::RecordAttendanceRequest,
    session_response::SessionResponse, start_session_request::StartSessionRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::session::{
    end_session::EndSessionCommand, get_attendance::GetAttendanceQuery,
    get_session::GetSessionQuery, list_sessions::ListSessionsQuery,
    record_attendance::RecordAttendanceCommand, start_session::StartSessionCommand,
};

pub fn session_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(start).get(list))
        .route("/{id}", get(get_by_id))
        .route("/{id}/end", post(end))
        .route(
            "/{id}/attendance",
            post(record_attendance).get(get_attendance),
        )
}

async fn start(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<StartSessionRequest>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = StartSessionCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id,
        class_id: payload.class_id,
        teacher_id: payload.teacher_id,
        notes: payload.notes,
    };

    let session = ctx
        .start_session
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SessionResponse::from(session),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<SessionResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListSessionsQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let sessions = ctx
        .list_sessions
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = sessions.into_iter().map(SessionResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetSessionQuery { session_id: id };

    let session = ctx
        .get_session
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SessionResponse::from(session),
        req_ctx.request_id,
    )))
}

async fn end(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<SessionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = EndSessionCommand { session_id: id };

    let session = ctx
        .end_session
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SessionResponse::from(session),
        req_ctx.request_id,
    )))
}

async fn record_attendance(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<RecordAttendanceRequest>,
) -> Result<Json<ApiResponse<AttendanceResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = RecordAttendanceCommand {
        tenant_id: req_ctx.tenant_id,
        session_id: id,
        student_id: payload.student_id,
        status: payload.status,
        checked_in_at: payload.checked_in_at,
        notes: payload.notes,
    };

    let attendance = ctx
        .record_attendance
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AttendanceResponse::from(attendance),
        req_ctx.request_id,
    )))
}

async fn get_attendance(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<AttendanceResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningSessionRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetAttendanceQuery { session_id: id };

    let records = ctx
        .get_attendance
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = records.into_iter().map(AttendanceResponse::from).collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}
