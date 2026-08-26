use axum::{
    Router,
    extract::{Json, Path, State},
    routing::{get, post},
};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};
use school_core::identity::application::auth::register_user::RegisterUserCommand;

use super::dto::{
    activate_master_request::ActivateMasterRequest,
    create_tenant_request::CreateTenantRequest,
    reset_credentials_request::ResetCredentialsRequest,
    system_responses::{
        ActivateMasterResponse, SystemAuditLogResponse, SystemOverviewResponse,
        TenantSummaryResponse,
    },
};

use axum::middleware;
use crate::middleware::auth_middleware;

pub fn system_routes(context: ApplicationContext) -> Router<ApplicationContext> {
    Router::new()
        .route("/login", post(system_login))
        .route("/maintenance-status", get(get_maintenance_status))
        .merge(
            Router::new()
                .route("/overview", get(get_system_overview))
                .route("/audit-logs", get(get_system_audit_logs))
                .route("/settings", get(get_system_settings).post(update_system_settings))
                .route("/tenants", get(list_tenants).post(create_tenant))
                .route("/tenants/{id}/activate-master", post(activate_master))
                .route("/tenants/{id}/reset-credentials", post(reset_credentials))
                .route("/tenants/{id}/toggle-status", post(toggle_tenant_status))
                .route("/tenants/{id}/impersonate", post(impersonate_tenant))
                .layer(middleware::from_fn_with_state(context, auth_middleware))
        )
}

/// System Overview Stats for Command Center Dashboard
async fn get_system_overview(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<SystemOverviewResponse>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let total_tenants: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM tenants")
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    let active_tenants: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM tenants WHERE is_active = true")
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    let total_students: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM students")
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    let total_teachers: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM teachers")
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    let total_classes: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM classes")
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    let total_guardians: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM guardians")
        .fetch_one(&ctx.pool)
        .await
        .unwrap_or(Some(0))
        .unwrap_or(0);

    let outbox_pending_events: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM outbox_events WHERE processed_at IS NULL"
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0);

    Ok(Json(ApiResponse::success(
        SystemOverviewResponse {
            total_tenants,
            active_tenants,
            total_students,
            total_teachers,
            total_classes,
            total_guardians,
            outbox_pending_events,
            server_engine: "Rust Axum Multi-Tenant Microservice".to_string(),
            rust_version: "1.82.0 (Stable Edition)".to_string(),
            database_status: "PostgreSQL 16 Multi-Tenant RLS Online".to_string(),
        },
        req_ctx.request_id,
    )))
}

/// Global System Audit Logs for Command Center
async fn get_system_audit_logs(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<SystemAuditLogResponse>>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let rows = sqlx::query!(
        r#"
        SELECT a.id, t.name as "tenant_name?", a.action as event_type, 
               COALESCE(a.resource || ' (' || a.decision || ')', a.decision) as "details!", 
               a.timestamp as created_at
        FROM audit_logs a
        LEFT JOIN tenants t ON t.id = a.tenant_id
        ORDER BY a.timestamp DESC
        LIMIT 50
        "#
    )
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let list = rows
        .into_iter()
        .map(|r| SystemAuditLogResponse {
            id: r.id,
            tenant_name: r.tenant_name.unwrap_or_else(|| "Global System".to_string()),
            event_type: r.event_type,
            details: r.details,
            created_at: r.created_at,
        })
        .collect();

    Ok(Json(ApiResponse::success(list, req_ctx.request_id)))
}

/// Toggle Tenant Active / Suspend Status
async fn toggle_tenant_status(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<bool>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let updated = sqlx::query_scalar!(
        "UPDATE tenants SET is_active = NOT is_active WHERE id = $1 RETURNING is_active",
        id
    )
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(
        school_core::common::error::ApplicationError::Infrastructure(
            school_core::common::error::InfrastructureError::Database(e),
        ),
        &req_ctx.request_id,
    ))?;

    Ok(Json(ApiResponse::success(updated, req_ctx.request_id)))
}

/// Impersonate / Direct Login as School Admin
async fn impersonate_tenant(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use school_core::identity::application::auth::authenticate_user::Claims;

    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    // Find any admin or user of this tenant
    let user_opt = sqlx::query!(
        r#"
        SELECT u.id, u.email, u.full_name, r.name as "role_name?"
        FROM users u
        LEFT JOIN user_roles ur ON ur.user_id = u.id
        LEFT JOIN roles r ON r.id = ur.role_id
        WHERE u.tenant_id = $1 AND u.is_active = true
        LIMIT 1
        "#,
        id
    )
    .fetch_optional(&ctx.pool)
    .await
    .unwrap_or(None);

    let (user_id, email, full_name, role_name) = match user_opt {
        Some(u) => (u.id, u.email, u.full_name, u.role_name.unwrap_or_else(|| "Admin".to_string())),
        None => {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::NotFound(
                    school_core::common::error_code::ErrorCode::ResourceNotFound,
                    "Tenant ini belum memiliki akun admin. Silakan buat akun master terlebih dahulu.".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }
    };

    let claims = Claims {
        sub: user_id.to_string(),
        tenant_id: id.to_string(),
        email: Some(email.clone()),
        full_name: Some(full_name.clone()),
        role: Some(role_name.clone()),
        exp: (chrono::Utc::now() + chrono::Duration::hours(12)).timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret("super_secret_jwt_key_123".as_ref()),
    )
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Internal(e.to_string()),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "token": token,
            "user_id": user_id,
            "email": email,
            "full_name": full_name,
            "role": role_name,
            "tenant_id": id
        }),
        req_ctx.request_id,
    )))
}

/// List all tenants and schools for system administration
#[utoipa::path(
    get,
    operation_id = "listSystemTenants",
    path = "/api/v1/system/tenants",
    responses(
        (status = 200, description = "Successfully listed tenants", body = inline(ApiResponse<Vec<TenantSummaryResponse>>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "SystemAdmin"
)]
async fn list_tenants(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<Vec<TenantSummaryResponse>>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let records = sqlx::query!(
        r#"
        SELECT t.id as tenant_id, t.name as tenant_name, t.is_active, t.created_at,
               s.name as "school_name?", s.npsn as "npsn?", s.dapodik_token as "dapodik_token?",
               (SELECT COUNT(*) FROM students WHERE tenant_id = t.id) as "student_count!",
               (SELECT COUNT(*) FROM teachers WHERE tenant_id = t.id) as "teacher_count!",
               (SELECT COUNT(*) FROM classes WHERE tenant_id = t.id) as "class_count!"
        FROM tenants t
        LEFT JOIN schools s ON s.tenant_id = t.id
        ORDER BY t.created_at DESC
        "#
    )
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(
        school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
        &req_ctx.request_id
    ))?;

    let mut response_data = Vec::new();
    for row in records {
        let is_dapodik = row.dapodik_token.as_ref().map(|t| !t.trim().is_empty()).unwrap_or(false);
        response_data.push(TenantSummaryResponse {
            tenant_id: row.tenant_id,
            tenant_name: row.tenant_name,
            school_name: row.school_name,
            npsn: row.npsn,
            is_active: row.is_active,
            created_at: row.created_at,
            server_status: if row.is_active { "🟢 Lancar".to_string() } else { "🔴 Suspend".to_string() },
            student_count: row.student_count,
            teacher_count: row.teacher_count,
            class_count: row.class_count,
            is_dapodik_connected: is_dapodik,
        });
    }

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}

async fn seed_tenant_standard_roles(pool: &sqlx::PgPool, tenant_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default)
        VALUES 
            (gen_random_uuid(), $1, 'Kepala Sekolah', 'Kepala Sekolah / Pimpinan Unit', 'WEB, ANDROID', true),
            (gen_random_uuid(), $1, 'Guru', 'Guru Pengajar & Wali Kelas', 'WEB, ANDROID', true),
            (gen_random_uuid(), $1, 'Operator/Staff', 'Operator Sekolah & Staf Administrasi', 'WEB', true),
            (gen_random_uuid(), $1, 'Bendahara', 'Bendahara & Pengelola Keuangan Sekolah', 'WEB, ANDROID', true),
            (gen_random_uuid(), $1, 'Siswa', 'Peserta Didik / Siswa', 'ANDROID', true),
            (gen_random_uuid(), $1, 'Wali Siswa', 'Orang Tua / Wali Murid', 'ANDROID', true)
        ON CONFLICT (tenant_id, name) DO NOTHING
        "#
    )
    .bind(tenant_id)
    .execute(pool)
    .await?;

    // Grant all domain permissions to 'Operator/Staff' and 'Kepala Sekolah' for this new tenant
    sqlx::query(
        r#"
        INSERT INTO role_permissions (role_id, permission)
        SELECT r.id, p.perm
        FROM roles r
        CROSS JOIN (
            VALUES
            ('Student.Read'), ('Student.Create'), ('Student.Update'), ('Student.Delete'),
            ('Teacher.Read'), ('Teacher.Create'), ('Teacher.Update'), ('Teacher.Delete'),
            ('Guardian.Read'), ('Guardian.Create'), ('Guardian.Update'), ('Guardian.Delete'),
            ('Staff.Read'), ('Staff.Create'), ('Staff.Update'), ('Staff.Delete'),
            ('Academic.Manage'),
            ('Learning.Curriculum.Create'), ('Learning.Curriculum.Read'), ('Learning.Curriculum.Update'), ('Learning.Curriculum.Delete'),
            ('Learning.Syllabus.Create'), ('Learning.Syllabus.Read'), ('Learning.Syllabus.Update'), ('Learning.Syllabus.Delete'),
            ('Learning.Material.Create'), ('Learning.Material.Read'), ('Learning.Material.Update'), ('Learning.Material.Delete'),
            ('Learning.Lesson.Create'), ('Learning.Lesson.Read'), ('Learning.Lesson.Update'), ('Learning.Lesson.Delete'),
            ('Learning.Session.Create'), ('Learning.Session.Read'), ('Learning.Session.Update'), ('Learning.Session.Delete'),
            ('Learning.Assignment.Create'), ('Learning.Assignment.Read'), ('Learning.Assignment.Update'), ('Learning.Assignment.Delete'),
            ('Learning.Quiz.Create'), ('Learning.Quiz.Read'), ('Learning.Quiz.Update'), ('Learning.Quiz.Delete'),
            ('Learning.Assessment.Configure'), ('Learning.Assessment.Read'),
            ('Learning.Progress.Read'), ('Learning.Progress.Update'),
            ('Learning.Achievement.Create'), ('Learning.Achievement.Read'), ('Learning.Achievement.Award'),
            ('Learning.Feed.Create'), ('Learning.Feed.Read'),
            ('Notification.Read'), ('Notification.Update'),
            ('Assessment.Input'), ('Assessment.Read'),
            ('Attendance.Record'), ('Attendance.Read'),
            ('School.Update')
        ) AS p(perm)
        WHERE r.tenant_id = $1 AND r.name IN ('Operator/Staff', 'Kepala Sekolah')
        ON CONFLICT DO NOTHING;
        "#
    )
    .bind(tenant_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Create a new Tenant & School with initialized master user
#[utoipa::path(
    post,
    operation_id = "createSystemTenant",
    path = "/api/v1/system/tenants",
    request_body = CreateTenantRequest,
    responses(
        (status = 200, description = "Successfully created tenant and master account", body = inline(ApiResponse<ActivateMasterResponse>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "SystemAdmin"
)]
async fn create_tenant(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateTenantRequest>,
) -> Result<Json<ApiResponse<ActivateMasterResponse>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string()
            ),
            &req_ctx.request_id,
        ));
    }

    let tenant_id = Uuid::new_v4();
    let school_id = Uuid::new_v4();

    // 1. Insert tenant
    sqlx::query("INSERT INTO tenants (id, name, is_active) VALUES ($1, $2, true)")
        .bind(tenant_id)
        .bind(&payload.school_name)
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id
        ))?;

    // 2. Insert school
    sqlx::query("INSERT INTO schools (id, tenant_id, name, npsn) VALUES ($1, $2, $3, $4)")
        .bind(school_id)
        .bind(tenant_id)
        .bind(&payload.school_name)
        .bind(&payload.npsn)
        .execute(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id
        ))?;

    // 3. Seed roles
    seed_tenant_standard_roles(&ctx.pool, tenant_id)
        .await
        .map_err(|e| ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
            &req_ctx.request_id
        ))?;

    // 4. Register master user
    let command = RegisterUserCommand {
        tenant_id,
        email: payload.master_email.clone(),
        password: payload.master_password,
        full_name: payload.master_full_name.clone(),
    };

    let user = ctx
        .register_user
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    // 5. Assign role
    let target_role_name = payload.master_role.unwrap_or_else(|| "Kepala Sekolah".to_string());
    let role_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM roles WHERE tenant_id = $1 AND name = $2 LIMIT 1"
    )
    .bind(tenant_id)
    .bind(&target_role_name)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(
        school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
        &req_ctx.request_id
    ))?;

    let mut assigned_role = "None".to_string();
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
        assigned_role = target_role_name;
    }

    let response_data = ActivateMasterResponse {
        user_id: user.id,
        email: payload.master_email,
        full_name: payload.master_full_name,
        assigned_role,
        message: format!("Tenant & akun master untuk {} berhasil dibuat.", payload.school_name),
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}

/// Activate a Master Account (Kepala Sekolah) for a tenant
#[utoipa::path(
    post,
    operation_id = "activateMasterAccount",
    path = "/api/v1/system/tenants/{id}/activate-master",
    request_body = ActivateMasterRequest,
    params(
        ("id" = Uuid, Path, description = "Tenant UUID")
    ),
    responses(
        (status = 200, description = "Successfully activated master account", body = inline(ApiResponse<ActivateMasterResponse>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "SystemAdmin"
)]
async fn activate_master(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<ActivateMasterRequest>,
) -> Result<Json<ApiResponse<ActivateMasterResponse>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string()
            ),
            &req_ctx.request_id,
        ));
    }

    // 1. Seed roles if not exist
    let _ = seed_tenant_standard_roles(&ctx.pool, tenant_id).await;

    // 2. Register the User
    let command = RegisterUserCommand {
        tenant_id,
        email: payload.email.clone(),
        password: payload.password,
        full_name: payload.full_name.clone(),
    };

    let user = ctx
        .register_user
        .execute(command)
        .await
        .map_err(|e| ApiError::new(e, &req_ctx.request_id))?;

    // 3. Find target role
    let target_role_name = payload.role_name.unwrap_or_else(|| "Kepala Sekolah".to_string());
    let role_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM roles WHERE tenant_id = $1 AND name = $2 LIMIT 1"
    )
    .bind(tenant_id)
    .bind(&target_role_name)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(
        school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
        &req_ctx.request_id
    ))?;

    let mut assigned_role = "None (Role missing)".to_string();

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
        
        assigned_role = target_role_name;
    }

    let response_data = ActivateMasterResponse {
        user_id: user.id,
        email: payload.email,
        full_name: payload.full_name,
        assigned_role,
        message: "Master account successfully provisioned.".to_string(),
    };

    Ok(Json(ApiResponse::success(
        response_data,
        req_ctx.request_id,
    )))
}

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};

/// Reset email or password for an operator or principal in a tenant
#[utoipa::path(
    post,
    operation_id = "resetTenantCredentials",
    path = "/api/v1/system/tenants/{id}/reset-credentials",
    request_body = ResetCredentialsRequest,
    params(
        ("id" = Uuid, Path, description = "Tenant UUID")
    ),
    responses(
        (status = 200, description = "Successfully reset credentials", body = inline(ApiResponse<serde_json::Value>)),
    ),
    security(
        ("Bearer" = [])
    ),
    tag = "SystemAdmin"
)]
async fn reset_credentials(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Path(tenant_id): Path<Uuid>,
    Json(payload): Json<ResetCredentialsRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string()
            ),
            &req_ctx.request_id,
        ));
    }

    // Check if user exists
    let user_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM users WHERE tenant_id = $1 AND email = $2"
    )
    .bind(tenant_id)
    .bind(&payload.current_email)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(
        school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
        &req_ctx.request_id
    ))?;

    let user_id = match user_id {
        Some(id) => id,
        None => {
            return Err(ApiError::new(
                school_core::common::error::ApplicationError::Domain(
                    school_core::common::error::DomainError::Validation(
                        "User not found for the provided email in this tenant".to_string()
                    )
                ),
                &req_ctx.request_id,
            ));
        }
    };

    let new_email = payload.new_email.unwrap_or(payload.current_email.clone());
    
    let mut password_hash = None;
    if let Some(pwd) = payload.new_password {
        if !pwd.is_empty() {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();
            password_hash = Some(
                argon2.hash_password(pwd.as_bytes(), &salt)
                .unwrap()
                .to_string()
            );
        }
    }

    if let Some(hash) = password_hash {
        sqlx::query("UPDATE users SET email = $1, password_hash = $2 WHERE id = $3 AND tenant_id = $4")
            .bind(&new_email)
            .bind(hash)
            .bind(user_id)
            .bind(tenant_id)
            .execute(&ctx.pool)
            .await
            .map_err(|e| ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id
            ))?;
    } else {
        sqlx::query("UPDATE users SET email = $1 WHERE id = $2 AND tenant_id = $3")
            .bind(&new_email)
            .bind(user_id)
            .bind(tenant_id)
            .execute(&ctx.pool)
            .await
            .map_err(|e| ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(school_core::common::error::InfrastructureError::Database(e)),
                &req_ctx.request_id
            ))?;
    }

    Ok(Json(ApiResponse::success(
        serde_json::json!({
            "message": "Credentials updated successfully",
            "email": new_email
        }),
        req_ctx.request_id,
    )))
}

use jsonwebtoken::{encode, EncodingKey, Header};
use school_core::identity::application::auth::authenticate_user::Claims;

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct SystemLoginRequest {
    pub email: String,
    pub password: String,
}

#[utoipa::path(
    post,
    operation_id = "systemAdminLogin",
    path = "/api/v1/system/login",
    request_body = SystemLoginRequest,
    responses(
        (status = 200, description = "Successfully logged in as system admin", body = inline(ApiResponse<serde_json::Value>)),
    ),
    tag = "SystemAdmin"
)]
async fn system_login(
    State(_ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<SystemLoginRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    if payload.email == "sysadmin@schoolos.com" && payload.password == "sysadmin123" {
        let claims = Claims {
            sub: Uuid::nil().to_string(),
            tenant_id: Uuid::nil().to_string(),
            email: Some(payload.email.clone()),
            full_name: Some("Super Admin".to_string()),
            role: Some("System Admin".to_string()),
            exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret("super_secret_jwt_key_123".as_ref()),
        )
        .unwrap();

        Ok(Json(ApiResponse::success(
            serde_json::json!({
                "token": token,
                "user": {
                    "id": Uuid::nil(),
                    "email": payload.email,
                    "full_name": "Super Admin"
                }
            }),
            req_ctx.request_id,
        )))
    } else {
        Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Invalid sysadmin credentials".to_string()
            ),
            &req_ctx.request_id,
        ))
    }
}

/// Public endpoint to check if maintenance mode is active
async fn get_maintenance_status(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let row = sqlx::query!(
        "SELECT value FROM system_settings WHERE key = 'maintenance'"
    )
    .fetch_optional(&ctx.pool)
    .await
    .ok()
    .flatten();

    let val = row.map(|r| r.value).unwrap_or_else(|| {
        serde_json::json!({
            "maintenance_mode": false,
            "maintenance_message": "Sistem sedang dalam peningkatan performa server terjadwal. Silakan kembali dalam beberapa menit."
        })
    });

    Ok(Json(ApiResponse::success(val, req_ctx.request_id)))
}

/// Get all global system settings (Super Admin only)
async fn get_system_settings(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    let rows = sqlx::query!("SELECT key, value FROM system_settings")
        .fetch_all(&ctx.pool)
        .await
        .unwrap_or_default();

    let mut map = serde_json::Map::new();
    for r in rows {
        map.insert(r.key, r.value);
    }

    Ok(Json(ApiResponse::success(serde_json::Value::Object(map), req_ctx.request_id)))
}

/// Update global system settings (Super Admin only)
async fn update_system_settings(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    if !req_ctx.actor.is_some_and(|a| a.id.is_nil()) {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Unauthorized(
                school_core::common::error_code::ErrorCode::AuthPermissionDenied,
                "Access denied. System Admin only.".to_string(),
            ),
            &req_ctx.request_id,
        ));
    }

    if let Some(obj) = payload.as_object() {
        for (k, v) in obj {
            let _ = sqlx::query!(
                r#"
                INSERT INTO system_settings (key, value, updated_at)
                VALUES ($1, $2, NOW())
                ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
                "#,
                k,
                v
            )
            .execute(&ctx.pool)
            .await;
        }
    }

    // Also record audit log for settings update
    let _ = sqlx::query!(
        r#"
        INSERT INTO audit_logs (id, tenant_id, actor_id, action, resource, decision, reason, timestamp)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
        "#,
        Uuid::new_v4(),
        Uuid::nil(),
        Uuid::nil(),
        "SYSTEM_SETTINGS_UPDATE",
        "system_settings",
        "ALLOWED",
        "Global server settings and policies updated by Super Admin"
    )
    .execute(&ctx.pool)
    .await;

    Ok(Json(ApiResponse::success(serde_json::json!({ "status": "updated" }), req_ctx.request_id)))
}

