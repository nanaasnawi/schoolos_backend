use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
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
    let auth_header = req.headers().get(header::AUTHORIZATION);
    let mut actor_opt = None;

    if let Some(auth_header) = auth_header {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                let token_data = decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(ctx.jwt_secret.as_bytes()),
                    &Validation::default(),
                );

                if let Ok(token_data) = token_data {
                    if let Ok(user_id) = Uuid::parse_str(&token_data.claims.sub) {
                        if let Ok(tenant_id) = Uuid::parse_str(&token_data.claims.tenant_id) {
                            // 1. In-memory Cache lookup for roles & permissions
                            let (roles, permissions) = if let Some(cached) = ctx
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

                            actor_opt = Some(Actor {
                                id: user_id,
                                tenant_id,
                                roles,
                                permissions,
                            });
                        } else {
                            tracing::error!(
                                "Failed to parse tenant_id: {}",
                                token_data.claims.tenant_id
                            );
                        }
                    } else {
                        tracing::error!("Failed to parse user_id: {}", token_data.claims.sub);
                    }
                } else {
                    tracing::error!("Failed to decode JWT token: {:?}", token_data.err());
                }
            }
        }
    }

    if let Some(actor) = actor_opt {
        // If not Super Admin, check if Global Maintenance Mode is active (using in-memory cache)
        if !actor.id.is_nil() {
            let is_maintenance = if let Some(cached) = ctx.auth_cache.get_maintenance_mode().await {
                cached
            } else {
                let row =
                    sqlx::query!("SELECT value FROM system_settings WHERE key = 'maintenance'")
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
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
        }

        req.extensions_mut().insert(actor);
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
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
