use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{DecodingKey, Validation, decode};
use school_core::authorization::domain::actor::Actor;
use school_core::identity::application::auth::authenticate_user::Claims;
use school_core::permission::domain::permission_registry::Permission;
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;

pub async fn auth_middleware(
    State(ctx): State<ApplicationContext>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get(header::AUTHORIZATION);
    let mut actor_opt = None;

    tracing::info!("auth_header present: {}", auth_header.is_some());
    if let Some(auth_header) = auth_header {
        let auth_str = auth_header.to_str();
        tracing::info!("auth_str ok: {:?}", auth_str);
        if let Ok(auth_str) = auth_str {
            let token_opt = auth_str.strip_prefix("Bearer ");
            tracing::info!("token stripped: {}", token_opt.is_some());
            if let Some(token) = token_opt {
                let jwt_secret = "super_secret_jwt_key_123";

                let token_data = decode::<Claims>(
                    token,
                    &DecodingKey::from_secret(jwt_secret.as_ref()),
                    &Validation::default(),
                );

                if let Ok(token_data) = token_data {
                    if let Ok(user_id) = Uuid::parse_str(&token_data.claims.sub) {
                        if let Ok(tenant_id) = Uuid::parse_str(&token_data.claims.tenant_id) {
                            let roles = ctx.role_repo.find_roles_by_user_id(user_id).await.unwrap_or_default();
                            let mut all_permissions = Vec::new();
                            for role in &roles {
                                if let Ok(perms) = ctx.role_repo.get_role_permissions(role.id).await {
                                    for p in perms {
                                        if !all_permissions.contains(&p) {
                                            all_permissions.push(p);
                                        }
                                    }
                                }
                            }

                            actor_opt = Some(Actor {
                                id: user_id,
                                tenant_id,
                                roles,
                                permissions: all_permissions,
                            });
                        } else {
                            tracing::error!("Failed to parse tenant_id: {}", token_data.claims.tenant_id);
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

    tracing::debug!("auth_middleware actor: {:?}", actor_opt);

    if let Some(actor) = actor_opt {
        // If not Super Admin, check if Global Maintenance Mode is active
        if !actor.id.is_nil() {
            let row = sqlx::query!(
                "SELECT value FROM system_settings WHERE key = 'maintenance'"
            )
            .fetch_optional(&ctx.pool)
            .await
            .ok()
            .flatten();

            if let Some(rec) = row {
                if rec.value.get("maintenance_mode").and_then(|v| v.as_bool()).unwrap_or(false) {
                    tracing::warn!("Rejecting request due to active Maintenance Mode for actor {}", actor.id);
                    return Err(StatusCode::SERVICE_UNAVAILABLE);
                }
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
