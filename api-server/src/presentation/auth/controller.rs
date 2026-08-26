use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};

use super::dto::{
    login_request::LoginRequest, login_response::LoginResponse,
    register_request::RegisterRequest, register_response::RegisterResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::identity::application::auth::{
    authenticate_user::AuthenticateUserCommand,
    register_user::RegisterUserCommand,
};

pub fn auth_routes(context: ApplicationContext) -> Router<ApplicationContext> {
    use axum::middleware;
    use crate::middleware::auth_middleware;

    Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .merge(
            Router::new()
                .route("/me", axum::routing::get(get_me))
                .route("/users", axum::routing::get(list_users))
                .layer(middleware::from_fn_with_state(context, auth_middleware)),
        )
}

/// Authenticate a user and return a JWT access token.
///
/// This endpoint verifies the user's email and password against the provided tenant.
/// Upon successful verification, it returns an access token valid for 24 hours.
#[utoipa::path(
    post,
    operation_id = "login",
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = inline(ApiResponse<LoginResponse>)),
        (status = 401, description = "Invalid credentials"),
        (status = 403, description = "Missing tenant ID")
    ),
    security(
        ()
    ),
    tag = "Auth"
)]
async fn login(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, ApiError> {
    // 1. Check if Global Maintenance Mode is active
    let maintenance_record = sqlx::query!(
        "SELECT value FROM system_settings WHERE key = 'maintenance'"
    )
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    if let Some(rec) = maintenance_record {
        if rec.value.get("maintenance_mode").and_then(|v| v.as_bool()).unwrap_or(false) {
            let msg = rec.value.get("maintenance_message")
                .and_then(|v| v.as_str())
                .unwrap_or("Sistem sedang dalam mode pemeliharaan oleh Super Admin.");
            
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Unauthorized(
                    school_core::common::error_code::ErrorCode::SystemMaintenance,
                    format!("Mode Pemeliharaan Aktif: {}", msg),
                ),
                &req_ctx.request_id,
            ));
        }
    }

    let identifier = payload.username.or(payload.email).unwrap_or_default();

    let command = AuthenticateUserCommand {
        tenant_id: req_ctx.tenant_id,
        email: identifier,
        password: payload.password,
    };

    let token = ctx
        .authenticate_user
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data = LoginResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: 86400,
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}

/// Register a new user.
#[utoipa::path(
    post,
    operation_id = "register",
    path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "Registration successful", body = inline(ApiResponse<RegisterResponse>)),
        (status = 400, description = "Invalid request")
    ),
    security(
        ()
    ),
    tag = "Auth"
)]
async fn register(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<RegisterResponse>>, ApiError> {
    let command = RegisterUserCommand {
        tenant_id: req_ctx.tenant_id,
        email: payload.email,
        password: payload.password,
        full_name: payload.full_name,
    };

    let user = ctx
        .register_user
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    if let Some(target_role_name) = payload.role {
        let role_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM roles WHERE tenant_id = $1 AND name = $2 LIMIT 1"
        )
        .bind(req_ctx.tenant_id)
        .bind(&target_role_name)
        .fetch_optional(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id
        ))?;

        if let Some(r_id) = role_id {
            sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(user.id)
                .bind(r_id)
                .execute(&ctx.pool)
                .await
                .map_err(|e| ApiError::new(
                    school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                    &req_ctx.request_id
                ))?;
        }
    }

    let response_data = RegisterResponse {
        message: "User registered successfully".to_string(),
        user_id: user.id.to_string(),
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct AuthUserDto {
    pub id: uuid::Uuid,
    pub email: String,
    pub full_name: String,
    pub role: String,
    pub is_active: bool,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(
    get,
    operation_id = "listUsers",
    path = "/api/v1/auth/users",
    responses(
        (status = 200, description = "Users listed successfully", body = inline(ApiResponse<Vec<AuthUserDto>>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Auth"
)]
async fn list_users(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<AuthUserDto>>>, ApiError> {
    let records = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.full_name, u.is_active, u.created_at,
               COALESCE(r.name, 'No Role') as role_name
        FROM users u
        LEFT JOIN user_roles ur ON u.id = ur.user_id
        LEFT JOIN roles r ON ur.role_id = r.id
        WHERE u.tenant_id = $1
        ORDER BY u.created_at DESC
        "#,
        req_ctx.tenant_id
    )
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let dtos = records.into_iter().map(|r| AuthUserDto {
        id: r.id,
        email: r.email,
        full_name: r.full_name,
        role: r.role_name.unwrap_or_default(),
        is_active: r.is_active,
        created_at: r.created_at,
    }).collect();

    Ok(Json(ApiResponse::success(dtos, req_ctx.request_id)))
}

/// Get the current authenticated user profile
#[utoipa::path(
    get,
    operation_id = "getCurrentUser",
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "Current user profile retrieved successfully", body = inline(ApiResponse<AuthUserDto>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "Auth"
)]
async fn get_me(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<AuthUserDto>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();

    if actor_id.is_nil() {
        return Ok(Json(ApiResponse::success(
            AuthUserDto {
                id: uuid::Uuid::nil(),
                email: "sysadmin@schoolos.com".to_string(),
                full_name: "Super Admin".to_string(),
                role: "System Administrator".to_string(),
                is_active: true,
                created_at: chrono::Utc::now(),
            },
            req_ctx.request_id,
        )));
    }

    let record = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.full_name, u.is_active, u.created_at,
               COALESCE(r.name, 'Administrator') as role_name
        FROM users u
        LEFT JOIN user_roles ur ON u.id = ur.user_id
        LEFT JOIN roles r ON ur.role_id = r.id
        WHERE u.id = $1
        LIMIT 1
        "#,
        actor_id
    )
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let dto = match record {
        Some(r) => AuthUserDto {
            id: r.id,
            email: r.email,
            full_name: r.full_name,
            role: r.role_name.unwrap_or_else(|| "Administrator".to_string()),
            is_active: r.is_active,
            created_at: r.created_at,
        },
        None => AuthUserDto {
            id: actor_id,
            email: "admin@schoolos.com".to_string(),
            full_name: "Admin Sistem".to_string(),
            role: "Administrator".to_string(),
            is_active: true,
            created_at: chrono::Utc::now(),
        },
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}
