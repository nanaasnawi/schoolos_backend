use axum::{
    extract::{OriginalUri, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use school_core::authorization::domain::actor::Actor;
use school_core::common::tenant_context::{with_system_rls_bypass, with_tenant_db_scope};
use school_core::identity::application::auth::authenticate_user::Claims;
use school_core::permission::domain::permission_registry::Permission;
use school_core::permission::domain::role::Role;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;

/// Derives the isolated JWT signing secret used exclusively for platform system-admin tokens.
pub fn system_jwt_signing_key(base_jwt_secret: &str) -> String {
    std::env::var("SYSTEM_JWT_SECRET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("__sysadmin_scope__:{base_jwt_secret}"))
}

#[derive(Clone)]
pub struct CachedActorData {
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
    pub is_active: bool,
    pub tenant_id: Uuid,
    pub cached_at: Instant,
}

#[derive(Clone)]
pub struct AuthCache {
    actors: Arc<RwLock<HashMap<Uuid, CachedActorData>>>,
    maintenance: Arc<RwLock<Option<(bool, Instant)>>>,
    ttl: Duration,
    maintenance_ttl: Duration,
}

impl Default for AuthCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthCache {
    pub fn new() -> Self {
        Self {
            actors: Arc::new(RwLock::new(HashMap::new())),
            maintenance: Arc::new(RwLock::new(None)),
            ttl: Duration::from_secs(30), // Fast 30s TTL so user deactivation takes effect rapidly
            maintenance_ttl: Duration::from_secs(15),
        }
    }

    pub async fn get_actor_entry(&self, user_id: &Uuid) -> Option<CachedActorData> {
        let read = self.actors.read().await;
        if let Some(data) = read.get(user_id) {
            if data.cached_at.elapsed() < self.ttl {
                return Some(data.clone());
            }
        }
        None
    }

    pub async fn set_actor_entry(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        is_active: bool,
        roles: Vec<Role>,
        permissions: Vec<Permission>,
    ) {
        let mut write = self.actors.write().await;
        if write.len() > 2000 {
            let ttl = self.ttl;
            write.retain(|_, v| v.cached_at.elapsed() < ttl);
        }
        write.insert(
            user_id,
            CachedActorData {
                roles,
                permissions,
                is_active,
                tenant_id,
                cached_at: Instant::now(),
            },
        );
    }

    pub async fn get_maintenance_mode(&self) -> Option<bool> {
        let read = self.maintenance.read().await;
        if let Some((mode, cached_at)) = *read {
            if cached_at.elapsed() < self.maintenance_ttl {
                return Some(mode);
            }
        }
        None
    }

    pub async fn set_maintenance_mode(&self, is_active: bool) {
        let mut write = self.maintenance.write().await;
        *write = Some((is_active, Instant::now()));
    }

    pub async fn invalidate_actor(&self, user_id: &Uuid) {
        let mut write = self.actors.write().await;
        write.remove(user_id);
    }

    pub async fn invalidate_all(&self) {
        let mut write_actors = self.actors.write().await;
        write_actors.clear();
        let mut write_maint = self.maintenance.write().await;
        *write_maint = None;
    }
}

fn json_auth_error(status: StatusCode, code: &str, message: &str) -> Response {
    let res_body = serde_json::json!({
        "success": false,
        "error": {
            "code": code,
            "message": message
        }
    });
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&res_body).unwrap_or_default(),
        ))
        .unwrap_or_else(|_| status.into_response())
}

fn actor_is_superadmin_role(actor: &Actor) -> bool {
    actor.roles.iter().any(|r| {
        let norm = r
            .name
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-', '_'], "");
        norm == "superadmin" || norm == "platformadmin" || norm == "systemadmin"
    })
}

/// Verifies a Bearer JWT access token (or dedicated system-admin token) and resolves an active `Actor`.
/// Enforces strict expiration for ALL roles (including Siswa) and checks user/tenant active state.
pub async fn authenticate_jwt_token(
    ctx: &ApplicationContext,
    token: &str,
) -> Result<(Actor, bool), (StatusCode, &'static str, &'static str)> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    validation.leeway = 5;

    let user_decode = decode::<Claims>(
        token,
        &DecodingKey::from_secret(ctx.jwt_secret.as_bytes()),
        &validation,
    );

    if let Ok(td) = user_decode {
        let claims = td.claims;
        if matches!(
            claims.token_type.as_deref(),
            Some("refresh") | Some("system_access")
        ) || claims.sysadmin == Some(true)
        {
            return Err((
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID_TOKEN",
                "Token tidak valid untuk akses API.",
            ));
        }

        let now_ts = chrono::Utc::now().timestamp() as usize;
        if claims.exp <= now_ts {
            return Err((
                StatusCode::UNAUTHORIZED,
                "AUTH_EXPIRED_TOKEN",
                "Sesi Anda telah berakhir (expired). Silakan muat ulang atau login kembali.",
            ));
        }

        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID_TOKEN",
                "Token sub tidak valid.",
            )
        })?;
        let tenant_id = Uuid::parse_str(&claims.tenant_id).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID_TOKEN",
                "Token tenant_id tidak valid.",
            )
        })?;

        if user_id.is_nil() || tenant_id.is_nil() {
            return Err((
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID_TOKEN",
                "Token pengguna reguler tidak valid.",
            ));
        }

        let cached = ctx.auth_cache.get_actor_entry(&user_id).await;
        let (roles, permissions) = if let Some(entry) = cached {
            if !entry.is_active || entry.tenant_id != tenant_id {
                ctx.auth_cache.invalidate_actor(&user_id).await;
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "AUTH_ACCOUNT_INACTIVE",
                    "Akun pengguna atau sekolah sedang nonaktif.",
                ));
            }
            (entry.roles, entry.permissions)
        } else {
            let lookup_res: Result<Option<(bool, bool)>, sqlx::Error> = with_system_rls_bypass(
                async {
                    let row = sqlx::query(
                        r#"
                        SELECT u.is_active AS user_active, t.is_active AS tenant_active
                        FROM users u
                        JOIN tenants t ON t.id = u.tenant_id
                        WHERE u.id = $1 AND u.tenant_id = $2
                        LIMIT 1
                        "#,
                    )
                    .bind(user_id)
                    .bind(tenant_id)
                    .fetch_optional(&ctx.pool)
                    .await?;

                    Ok(row.map(|r| {
                        (
                            r.try_get::<bool, _>("user_active").unwrap_or(false),
                            r.try_get::<bool, _>("tenant_active").unwrap_or(false),
                        )
                    }))
                },
            )
            .await;

            let (user_active, tenant_active) = match lookup_res {
                Ok(Some(flags)) => flags,
                Ok(None) => (false, false),
                Err(e) => {
                    tracing::error!("Auth active-state lookup DB error: {:?}", e);
                    return Err((
                        StatusCode::SERVICE_UNAVAILABLE,
                        "AUTH_SERVICE_UNAVAILABLE",
                        "Gagal memverifikasi status sesi pengguna.",
                    ));
                }
            };

            if !user_active || !tenant_active {
                ctx.auth_cache.invalidate_actor(&user_id).await;
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "AUTH_ACCOUNT_INACTIVE",
                    "Akun Anda dinonaktifkan, dihapus, atau telah dimutasi keluar.",
                ));
            }

            let (r, p) = with_system_rls_bypass(async {
                let r = ctx
                    .role_repo
                    .find_roles_by_user_id(user_id)
                    .await
                    .unwrap_or_default();
                let p = ctx
                    .role_repo
                    .find_permissions_by_user_id(user_id)
                    .await
                    .unwrap_or_default();
                (r, p)
            })
            .await;

            ctx.auth_cache
                .set_actor_entry(user_id, tenant_id, true, r.clone(), p.clone())
                .await;
            (r, p)
        };

        return Ok((
            Actor {
                id: user_id,
                tenant_id,
                roles,
                permissions,
            },
            false,
        ));
    }

    if let Err(ref err) = user_decode {
        if matches!(err.kind(), jsonwebtoken::errors::ErrorKind::ExpiredSignature) {
            return Err((
                StatusCode::UNAUTHORIZED,
                "AUTH_EXPIRED_TOKEN",
                "Sesi Anda telah berakhir (expired). Silakan refresh atau login kembali.",
            ));
        }
    }

    // Attempt verification ONLY for dedicated system_access tokens signed with isolated system key
    let sys_secret = system_jwt_signing_key(&ctx.jwt_secret);
    if let Ok(td_sys) = decode::<Claims>(
        token,
        &DecodingKey::from_secret(sys_secret.as_bytes()),
        &validation,
    ) {
        let claims = td_sys.claims;
        if claims.sysadmin == Some(true)
            && claims.token_type.as_deref() == Some("system_access")
            && Uuid::parse_str(&claims.sub).ok() == Some(Uuid::nil())
        {
            let now_ts = chrono::Utc::now().timestamp() as usize;
            if claims.exp <= now_ts {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    "AUTH_EXPIRED_TOKEN",
                    "Sesi System Admin telah berakhir (expired).",
                ));
            }
            return Ok((
                Actor {
                    id: Uuid::nil(),
                    tenant_id: Uuid::nil(),
                    roles: vec![Role {
                        id: Uuid::nil(),
                        tenant_id: Uuid::nil(),
                        name: "SuperAdmin".to_string(),
                        is_system_default: true,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    }],
                    permissions: vec![
                        Permission::AcademicManage,
                        Permission::StudentRead,
                        Permission::StudentUpdate,
                        Permission::TeacherRead,
                        Permission::TeacherUpdate,
                        Permission::LearningCurriculumRead,
                        Permission::LearningCurriculumUpdate,
                    ],
                },
                true,
            ));
        }
    }

    Err((
        StatusCode::UNAUTHORIZED,
        "AUTH_INVALID_TOKEN",
        "Token autentikasi tidak valid. Silakan login kembali.",
    ))
}

pub async fn auth_middleware(
    State(ctx): State<ApplicationContext>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut token_opt: Option<String> = None;

    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            let trimmed = auth_str.trim();
            if let Some(token) = trimmed
                .strip_prefix("Bearer ")
                .or_else(|| trimmed.strip_prefix("bearer "))
            {
                token_opt = Some(token.trim().to_string());
            }
        }
    }

    // Fail-fast #1: Reject unauthenticated requests without Authorization Bearer header
    // (URL query `?token=` JWT parsing intentionally removed; SSE uses short-lived one-time `?ticket=`.)
    let token = match token_opt {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Ok(json_auth_error(
                StatusCode::UNAUTHORIZED,
                "AUTH_UNAUTHORIZED",
                "Header otorisasi tidak ditemukan. Silakan login kembali.",
            ));
        }
    };

    let (actor, is_verified_sysadmin) = match authenticate_jwt_token(&ctx, &token).await {
        Ok(res) => res,
        Err((status, code, msg)) => return Ok(json_auth_error(status, code, msg)),
    };

    let is_superadmin = is_verified_sysadmin || actor_is_superadmin_role(&actor);

    // Enforce strict tenant isolation against spoofed `x-tenant-id` header
    let header_tenant_id = req
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| Uuid::parse_str(s).ok())
        .filter(|id| !id.is_nil());

    if let Some(hdr_tid) = header_tenant_id {
        if !is_superadmin && hdr_tid != actor.tenant_id {
            tracing::warn!(
                actor_id = %actor.id,
                token_tenant = %actor.tenant_id,
                header_tenant = %hdr_tid,
                "Rejected cross-tenant X-Tenant-ID header mismatch"
            );
            return Ok(json_auth_error(
                StatusCode::FORBIDDEN,
                "AUTH_TENANT_MISMATCH",
                "Akses lintas tenant sekolah tidak diizinkan.",
            ));
        }
    }

    let effective_tenant_id = if is_superadmin {
        header_tenant_id.unwrap_or(actor.tenant_id)
    } else {
        actor.tenant_id
    };

    // Global Maintenance Mode check using fast in-memory cache
    if !is_verified_sysadmin {
        let is_maintenance = if let Some(cached) = ctx.auth_cache.get_maintenance_mode().await {
            cached
        } else {
            let row = with_system_rls_bypass(
                sqlx::query!("SELECT value FROM system_settings WHERE key = 'maintenance'")
                    .fetch_optional(&ctx.pool),
            )
            .await
            .ok()
            .flatten();

            let active = row
                .and_then(|rec| rec.value.get("maintenance_mode").and_then(|v| v.as_bool()))
                .unwrap_or(false);
            ctx.auth_cache.set_maintenance_mode(active).await;
            active
        };

        if is_maintenance {
            tracing::warn!(
                "Rejecting request due to active Maintenance Mode for actor {}",
                actor.id
            );
            return Ok(json_auth_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "MAINTENANCE_MODE_ACTIVE",
                "Sistem sedang dalam mode pemeliharaan (maintenance).",
            ));
        }
    }

    let orig_path = req
        .extensions()
        .get::<OriginalUri>()
        .map(|u| u.0.path().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    req.extensions_mut().insert(actor);

    let use_system_bypass = is_superadmin
        && (orig_path.starts_with("/api/v1/system")
            || orig_path.starts_with("/api/v1/tenants")
            || effective_tenant_id.is_nil());

    if use_system_bypass {
        Ok(with_system_rls_bypass(next.run(req)).await)
    } else {
        Ok(with_tenant_db_scope(effective_tenant_id, next.run(req)).await)
    }
}

/// Check whether the requesting actor has a required permission.
/// Call this at the top of any protected handler.
pub fn require_permission(actor: &Option<Actor>, permission: Permission) -> Result<(), StatusCode> {
    match actor {
        Some(a) if a.has_permission(&permission) => Ok(()),
        _ => Err(StatusCode::FORBIDDEN),
    }
}
