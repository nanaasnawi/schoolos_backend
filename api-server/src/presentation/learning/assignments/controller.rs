use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use super::dto::{
    assignment_response::AssignmentResponse, create_assignment_request::CreateAssignmentRequest,
    grade_submission_request::GradeSubmissionRequest, submission_response::SubmissionResponse,
    submit_assignment_request::SubmitAssignmentRequest,
    update_assignment_request::UpdateAssignmentRequest,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::learning::application::assignment::{
    archive_assignment::ArchiveAssignmentCommand, close_assignment::CloseAssignmentCommand,
    create_assignment::CreateAssignmentCommand, delete_assignment::DeleteAssignmentCommand,
    get_assignment::GetAssignmentQuery, get_submissions::GetSubmissionsQuery,
    grade_submission::GradeSubmissionCommand, list_assignments::ListAssignmentsQuery,
    publish_assignment::PublishAssignmentCommand, submit_assignment::SubmitAssignmentCommand,
    update_assignment::UpdateAssignmentCommand,
};

pub fn assignment_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", post(create).get(list))
        .route("/{id}", get(get_by_id).patch(update).delete(delete))
        .route("/{id}/publish", post(publish))
        .route("/{id}/close", post(close))
        .route("/{id}/archive", post(archive))
        .route("/{id}/submit", post(submit))
        .route("/{id}/submissions", get(get_submissions))
        .route("/{id}/submissions/{submission_id}/grade", post(grade))
}

async fn create(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateAssignmentRequest>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentCreate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CreateAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        lesson_id: payload.lesson_id,
        title: payload.title,
        description: payload.description,
        instructions: payload.instructions,
        max_score: payload.max_score.unwrap_or(100),
        due_at: payload.due_at,
        assignment_type: payload.assignment_type,
    };

    let assignment = ctx
        .create_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn list(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<AssignmentResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = ListAssignmentsQuery {
        tenant_id: req_ctx.tenant_id,
    };

    let assignments = ctx
        .list_assignments
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = assignments
        .into_iter()
        .map(AssignmentResponse::from)
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn get_by_id(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetAssignmentQuery { assignment_id: id };

    let assignment = ctx
        .get_assignment
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn update(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAssignmentRequest>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = UpdateAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        title: payload.title,
        description: payload.description,
        instructions: payload.instructions,
        max_score: payload.max_score,
        due_at: payload.due_at,
    };

    let assignment = ctx
        .update_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn publish(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = PublishAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .publish_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn close(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = CloseAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .close_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn archive(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<AssignmentResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let command = ArchiveAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
    };

    let assignment = ctx
        .archive_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        AssignmentResponse::from(assignment),
        req_ctx.request_id,
    )))
}

async fn delete(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentDelete).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    let command = DeleteAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        deleted_by: actor_id,
    };

    ctx.delete_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success((), req_ctx.request_id)))
}

async fn submit(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
    Json(payload): Json<SubmitAssignmentRequest>,
) -> Result<Json<ApiResponse<SubmissionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let student_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let command = SubmitAssignmentCommand {
        tenant_id: req_ctx.tenant_id,
        assignment_id: id,
        student_id,
        content: payload.content,
        file_url: payload.file_url,
    };

    let submission = ctx
        .submit_assignment
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SubmissionResponse::from(submission),
        req_ctx.request_id,
    )))
}

async fn get_submissions(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<SubmissionResponse>>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentRead).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let query = GetSubmissionsQuery { assignment_id: id };

    let submissions = ctx
        .get_submissions
        .execute(query)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let items = submissions
        .into_iter()
        .map(SubmissionResponse::from)
        .collect();

    Ok(Json(ApiResponse::success(items, req_ctx.request_id)))
}

async fn grade(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path((_id, submission_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<GradeSubmissionRequest>,
) -> Result<Json<ApiResponse<SubmissionResponse>>, ApiError> {
    use crate::middleware::require_permission;
    use school_core::permission::domain::permission_registry::Permission;
    require_permission(&req_ctx.actor, Permission::LearningAssignmentUpdate).map_err(|_| {
        ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Insufficient permissions".to_string(),
            ),
            &req_ctx.request_id,
        )
    })?;

    let grader_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    let command = GradeSubmissionCommand {
        tenant_id: req_ctx.tenant_id,
        submission_id,
        score: payload.score,
        feedback: payload.feedback,
        graded_by: grader_id,
    };

    let submission = ctx
        .grade_submission
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    Ok(Json(ApiResponse::success(
        SubmissionResponse::from(submission),
        req_ctx.request_id,
    )))
}
