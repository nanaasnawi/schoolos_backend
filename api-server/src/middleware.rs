use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use school_core::authorization::domain::actor::Actor;
use school_core::identity::application::auth::authenticate_user::Claims;
use school_core::permission::domain::permission_registry::Permission;
use school_core::permission::domain::role::Role;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;

#[derive(Clone)]
pub struct CachedActorData {
    pub roles: Vec<Role>,
    pub permissions: Vec<Permission>,
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
            ttl: Duration::from_secs(60), // Cache user roles & permissions for 60s
            maintenance_ttl: Duration::from_secs(15), // Cache maintenance status for 15s
        }
    }

    pub async fn get_actor_roles_and_permissions(
        &self,
        user_id: &Uuid,
    ) -> Option<(Vec<Role>, Vec<Permission>)> {
        let read = self.actors.read().await;
        if let Some(data) = read.get(user_id) {
            if data.cached_at.elapsed() < self.ttl {
                return Some((data.roles.clone(), data.permissions.clone()));
            }
        }
        None
    }

    pub async fn set_actor_roles_and_permissions(
        &self,
        user_id: Uuid,
        roles: Vec<Role>,
        permissions: Vec<Permission>,
    ) {
        let mut write = self.actors.write().await;
        if write.len() > 1000 {
            let ttl = self.ttl;
            write.retain(|_, v| v.cached_at.elapsed() < ttl);
        }
        write.insert(
            user_id,
            CachedActorData {
                roles,
                permissions,
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

pub async fn auth_middleware(
    State(ctx): State<ApplicationContext>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut token_opt: Option<String> = None;

    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                token_opt = Some(token.trim().to_string());
            }
        }
    } else if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            if parts.next() == Some("token") {
                if let Some(val) = parts.next() {
                    token_opt = Some(val.trim().to_string());
                    break;
                }
            }
        }
    }

    // Fail-fast #1: If no token provided on protected route, return 401 JSON immediately
    let token = match token_opt {
        Some(t) if !t.is_empty() => t,
        _ => {
            let res_body = serde_json::json!({
                "success": false,
                "error": {
                    "code": "AUTH_UNAUTHORIZED",
                    "message": "Header otorisasi atau token tidak ditemukan. Silakan login kembali."
                }
            });
            return Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&res_body).unwrap_or_default(),
                ))
                .unwrap_or_else(|_| StatusCode::UNAUTHORIZED.into_response()));
        }
    };

    // Fail-fast #2: Decode and validate JWT token signature and expiration
    // Reject immediately at the outer layer before touching Postgres connection pool
    let mut validation = Validation::default();
    validation.validate_exp = true;
    validation.leeway = 5;

    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(ctx.jwt_secret.as_bytes()),
        &validation,
    );

    let claims = match token_data {
        Err(err) => {
            let (code, msg) = match err.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => (
                    "AUTH_EXPIRED_TOKEN",
                    "Sesi Anda telah berakhir (expired). Silakan login kembali.",
                ),
                _ => (
                    "AUTH_INVALID_TOKEN",
                    "Token autentikasi tidak valid. Silakan login kembali.",
                ),
            };
            tracing::warn!(
                "Fail-fast JWT rejection at gateway: {} (err: {:?})",
                msg,
                err.kind()
            );
            let res_body = serde_json::json!({
                "success": false,
                "error": {
                    "code": code,
                    "message": msg
                }
            });
            return Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&res_body).unwrap_or_default(),
                ))
                .unwrap_or_else(|_| StatusCode::UNAUTHORIZED.into_response()));
        }
        Ok(td) => {
            // Additional wall-clock timestamp check against claims.exp
            let now_ts = chrono::Utc::now().timestamp() as usize;
            if td.claims.exp <= now_ts {
                tracing::warn!(
                    "Fail-fast JWT rejection: token expired (exp: {}, now: {})",
                    td.claims.exp,
                    now_ts
                );
                let res_body = serde_json::json!({
                    "success": false,
                    "error": {
                        "code": "AUTH_EXPIRED_TOKEN",
                        "message": "Sesi Anda telah berakhir (expired). Silakan login kembali."
                    }
                });
                return Ok(Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(
                        serde_json::to_string(&res_body).unwrap_or_default(),
                    ))
                    .unwrap_or_else(|_| StatusCode::UNAUTHORIZED.into_response()));
            }
            td.claims
        }
    };

    // Fail-fast #3: Validate subject and tenant_id UUIDs
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            tracing::error!("Fail-fast: Invalid user_id in JWT subject: {}", claims.sub);
            let res_body = serde_json::json!({
                "success": false,
                "error": {
                    "code": "AUTH_INVALID_TOKEN",
                    "message": "Token sub tidak valid."
                }
            });
            return Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&res_body).unwrap_or_default(),
                ))
                .unwrap_or_else(|_| StatusCode::UNAUTHORIZED.into_response()));
        }
    };

    let tenant_id = match Uuid::parse_str(&claims.tenant_id) {
        Ok(id) => id,
        Err(_) => {
            tracing::error!(
                "Fail-fast: Invalid tenant_id in JWT claims: {}",
                claims.tenant_id
            );
            let res_body = serde_json::json!({
                "success": false,
                "error": {
                    "code": "AUTH_INVALID_TOKEN",
                    "message": "Token tenant_id tidak valid."
                }
            });
            return Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&res_body).unwrap_or_default(),
                ))
                .unwrap_or_else(|_| StatusCode::UNAUTHORIZED.into_response()));
        }
    };

    // 1. In-memory Cache lookup for roles & permissions
    let (roles, permissions) = if user_id.is_nil() {
        (
            vec![Role {
                id: Uuid::nil(),
                tenant_id: Uuid::nil(),
                name: "SuperAdmin".to_string(),
                is_system_default: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }],
            vec![
                Permission::AcademicManage,
                Permission::StudentRead,
                Permission::StudentUpdate,
                Permission::TeacherRead,
                Permission::TeacherUpdate,
                Permission::LearningCurriculumRead,
                Permission::LearningCurriculumUpdate,
            ],
        )
    } else if let Some(cached) = ctx
        .auth_cache
        .get_actor_roles_and_permissions(&user_id)
        .await
    {
        cached
    } else {
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
        ctx.auth_cache
            .set_actor_roles_and_permissions(user_id, r.clone(), p.clone())
            .await;
        (r, p)
    };

    let actor = Actor {
        id: user_id,
        tenant_id,
        roles,
        permissions,
    };

    // 2. Global Maintenance Mode check using fast in-memory cache
    if !actor.id.is_nil() {
        let is_maintenance = if let Some(cached) = ctx.auth_cache.get_maintenance_mode().await {
            cached
        } else {
            let row = sqlx::query!("SELECT value FROM system_settings WHERE key = 'maintenance'")
                .fetch_optional(&ctx.pool)
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
            let res_body = serde_json::json!({
                "success": false,
                "error": {
                    "code": "MAINTENANCE_MODE_ACTIVE",
                    "message": "Sistem sedang dalam mode pemeliharaan (maintenance)."
                }
            });
            return Ok(Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&res_body).unwrap_or_default(),
                ))
                .unwrap_or_else(|_| StatusCode::SERVICE_UNAVAILABLE.into_response()));
        }
    }

    req.extensions_mut().insert(actor);
    Ok(next.run(req).await)
}

/// Check whether the requesting actor has a required permission.
/// Call this at the top of any protected handler.
pub fn require_permission(actor: &Option<Actor>, permission: Permission) -> Result<(), StatusCode> {
    match actor {
        Some(a) if a.has_permission(&permission) => Ok(()),
        _ => Err(StatusCode::FORBIDDEN),
    }
}
