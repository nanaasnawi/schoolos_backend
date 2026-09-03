use axum::{
    Router,
    extract::{Json, State},
    routing::post,
};

use super::dto::{
    login_request::LoginRequest, login_response::LoginResponse,
    qr_login_request::{
        BatchGenerateQrBadgesRequest, BatchGenerateQrItemDto, GenerateQrBadgeRequest,
        QrLoginRequest, UserQrStatusDto,
    },
    register_request::RegisterRequest, register_response::RegisterResponse,
};
use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::identity::application::auth::{
    authenticate_user::AuthenticateUserCommand,
    generate_qr_token::GenerateQrTokenCommand,
    register_user::RegisterUserCommand,
};

pub fn auth_routes(context: ApplicationContext) -> Router<ApplicationContext> {
    use axum::middleware;
    use crate::middleware::auth_middleware;

    Router::new()
        .route("/login", post(login))
        .route("/qr-login", post(qr_login))
        .route("/register", post(register))
        .merge(
            Router::new()
                .route("/me", axum::routing::get(get_me))
                .route("/users", axum::routing::get(list_users))
                .route("/qr-tokens/generate", post(generate_qr_token_endpoint))
                .route("/qr-tokens/batch-generate", post(batch_generate_qr_tokens_endpoint))
                .route("/qr-tokens/users", axum::routing::get(list_users_qr_status))
                .route("/qr-tokens/my-badge", axum::routing::get(get_my_qr_badge))
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
        user_id: None,
        tenant_id: None,
        name: None,
        email: None,
        role: None,
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

/// Authenticate a user via Zero-Password QR Code / Badge Token
#[utoipa::path(
    post,
    operation_id = "qrLogin",
    path = "/api/v1/auth/qr-login",
    request_body = QrLoginRequest,
    responses(
        (status = 200, description = "Login successful", body = inline(ApiResponse<LoginResponse>)),
        (status = 401, description = "Invalid or expired QR token")
    ),
    security(()),
    tag = "Auth"
)]
async fn qr_login(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<QrLoginRequest>,
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

    let result = ctx
        .authenticate_qr_token
        .execute(&payload.token)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let response_data = LoginResponse {
        access_token: result.token,
        token_type: "Bearer".to_string(),
        expires_in: result.expires_in,
        user_id: Some(result.user_id.to_string()),
        tenant_id: Some(result.tenant_id.to_string()),
        name: Some(result.full_name),
        email: Some(result.email),
        role: Some(result.role),
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct QrBadgeDetailDto {
    pub id: uuid::Uuid,
    pub raw_token: String,
    pub user_id: uuid::Uuid,
    pub tenant_id: uuid::Uuid,
    pub token_type: String,
    pub label: String,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Generate a new QR Badge Token for a user (Admin/Operator)
#[utoipa::path(
    post,
    operation_id = "generateQrToken",
    path = "/api/v1/auth/qr-tokens/generate",
    request_body = GenerateQrBadgeRequest,
    responses(
        (status = 200, description = "QR Badge generated successfully", body = inline(ApiResponse<QrBadgeDetailDto>)),
    ),
    security(("Bearer" = [])),
    tag = "Auth"
)]
async fn generate_qr_token_endpoint(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<GenerateQrBadgeRequest>,
) -> Result<Json<ApiResponse<QrBadgeDetailDto>>, ApiError> {
    let command = GenerateQrTokenCommand {
        tenant_id: req_ctx.tenant_id,
        user_id: payload.user_id,
        token_type: payload.token_type,
        label: payload.label,
        expires_in_days: payload.expires_in_days,
    };

    let generated = ctx
        .generate_qr_token
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let dto = QrBadgeDetailDto {
        id: generated.id,
        raw_token: generated.raw_token,
        user_id: generated.user_id,
        tenant_id: generated.tenant_id,
        token_type: generated.token_type,
        label: generated.label,
        expires_at: generated.expires_at,
        created_at: generated.created_at,
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}

/// Get current authenticated user's active QR Badge
#[utoipa::path(
    get,
    operation_id = "getMyQrBadge",
    path = "/api/v1/auth/qr-tokens/my-badge",
    responses(
        (status = 200, description = "QR Badge retrieved successfully", body = inline(ApiResponse<QrBadgeDetailDto>)),
    ),
    security(("Bearer" = [])),
    tag = "Auth"
)]
async fn get_my_qr_badge(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<QrBadgeDetailDto>>, ApiError> {
    let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
    if actor_id.is_nil() {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthInvalidCredentials,
                "Unauthorized actor".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let command = GenerateQrTokenCommand {
        tenant_id: req_ctx.tenant_id,
        user_id: actor_id,
        token_type: Some("BADGE".to_string()),
        label: Some("Kartu Identitas Digital".to_string()),
        expires_in_days: None,
    };

    let generated = ctx
        .generate_qr_token
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    let dto = QrBadgeDetailDto {
        id: generated.id,
        raw_token: generated.raw_token,
        user_id: generated.user_id,
        tenant_id: generated.tenant_id,
        token_type: generated.token_type,
        label: generated.label,
        expires_at: generated.expires_at,
        created_at: generated.created_at,
    };

    Ok(Json(ApiResponse::success(dto, req_ctx.request_id)))
}

/// List all users eligible for mobile access with their QR token status
#[utoipa::path(
    get,
    operation_id = "listUsersQrStatus",
    path = "/api/v1/auth/qr-tokens/users",
    responses(
        (status = 200, description = "Users with QR status retrieved successfully", body = inline(ApiResponse<Vec<UserQrStatusDto>>)),
    ),
    security(("Bearer" = [])),
    tag = "Auth"
)]
async fn list_users_qr_status(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<UserQrStatusDto>>>, ApiError> {
    let rows = sqlx::query!(
        r#"
        SELECT 
            u.id, 
            u.email, 
            u.full_name, 
            u.is_active,
            COALESCE(r.name, 'No Role') as role_name,
            COALESCE(s.nisn, t.nip, g.phone_number, '') as "identifier!",
            c.name as "class_name?",
            (SELECT q.id FROM user_qr_tokens q WHERE q.user_id = u.id AND q.is_active = true ORDER BY q.created_at DESC LIMIT 1) as active_token_id,
            (SELECT q.label FROM user_qr_tokens q WHERE q.user_id = u.id AND q.is_active = true ORDER BY q.created_at DESC LIMIT 1) as active_token_label,
            (SELECT q.created_at FROM user_qr_tokens q WHERE q.user_id = u.id AND q.is_active = true ORDER BY q.created_at DESC LIMIT 1) as token_created_at,
            (SELECT q.last_used_at FROM user_qr_tokens q WHERE q.user_id = u.id AND q.is_active = true ORDER BY q.created_at DESC LIMIT 1) as token_last_used_at
        FROM users u
        LEFT JOIN user_roles ur ON u.id = ur.user_id
        LEFT JOIN roles r ON ur.role_id = r.id
        LEFT JOIN students s ON s.user_id = u.id
        LEFT JOIN teachers t ON t.user_id = u.id
        LEFT JOIN guardians g ON g.user_id = u.id
        LEFT JOIN enrollments en ON en.student_id = s.id AND en.status = 'ACTIVE'
        LEFT JOIN classes c ON c.id = en.class_id
        WHERE u.tenant_id = $1
        ORDER BY u.created_at DESC
        "#,
        req_ctx.tenant_id
    )
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let dtos = rows.into_iter().map(|r| UserQrStatusDto {
        id: r.id,
        email: r.email,
        full_name: r.full_name,
        role: r.role_name.unwrap_or_default(),
        is_active: r.is_active,
        identifier: if r.identifier.is_empty() { None } else { Some(r.identifier) },
        class_name: r.class_name,
        has_active_token: r.active_token_id.is_some(),
        active_token_label: r.active_token_label,
        token_created_at: r.token_created_at,
        token_last_used_at: r.token_last_used_at,
    }).collect();

    Ok(Json(ApiResponse::success(dtos, req_ctx.request_id)))
}

/// Batch generate QR tokens for multiple users
#[utoipa::path(
    post,
    operation_id = "batchGenerateQrTokens",
    path = "/api/v1/auth/qr-tokens/batch-generate",
    request_body = BatchGenerateQrBadgesRequest,
    responses(
        (status = 200, description = "Batch QR badges generated successfully", body = inline(ApiResponse<Vec<BatchGenerateQrItemDto>>)),
    ),
    security(("Bearer" = [])),
    tag = "Auth"
)]
async fn batch_generate_qr_tokens_endpoint(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<BatchGenerateQrBadgesRequest>,
) -> Result<Json<ApiResponse<Vec<BatchGenerateQrItemDto>>>, ApiError> {
    let mut results = Vec::new();

    for user_id in payload.user_ids {
        let command = GenerateQrTokenCommand {
            tenant_id: req_ctx.tenant_id,
            user_id,
            token_type: payload.token_type.clone(),
            label: payload.label.clone(),
            expires_in_days: payload.expires_in_days,
        };

        if let Ok(generated) = ctx.generate_qr_token.execute(command).await {
            let user_info = sqlx::query!(
                r#"
                SELECT 
                    u.full_name, 
                    u.email, 
                    COALESCE(r.name, 'No Role') as role_name,
                    COALESCE(s.nisn, t.nip, g.phone_number, '') as "identifier!",
                    c.name as "class_name?"
                FROM users u
                LEFT JOIN user_roles ur ON u.id = ur.user_id
                LEFT JOIN roles r ON ur.role_id = r.id
                LEFT JOIN students s ON s.user_id = u.id
                LEFT JOIN teachers t ON t.user_id = u.id
                LEFT JOIN guardians g ON g.user_id = u.id
                LEFT JOIN enrollments en ON en.student_id = s.id AND en.status = 'ACTIVE'
                LEFT JOIN classes c ON c.id = en.class_id
                WHERE u.id = $1 AND u.tenant_id = $2
                LIMIT 1
                "#,
                user_id,
                req_ctx.tenant_id
            )
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();

            let (full_name, email, role, identifier, class_name) = match user_info {
                Some(u) => (
                    u.full_name,
                    u.email,
                    u.role_name.unwrap_or_default(),
                    if u.identifier.is_empty() { None } else { Some(u.identifier) },
                    u.class_name,
                ),
                None => ("Pengguna".to_string(), String::new(), "User".to_string(), None, None),
            };


            results.push(BatchGenerateQrItemDto {
                id: generated.id,
                user_id: generated.user_id,
                raw_token: generated.raw_token,
                full_name,
                email,
                role,
                identifier,
                class_name,
                token_type: generated.token_type,
                label: generated.label,
                expires_at: generated.expires_at,
                created_at: generated.created_at,
            });
        }
    }

    Ok(Json(ApiResponse::success(results, req_ctx.request_id)))
}


