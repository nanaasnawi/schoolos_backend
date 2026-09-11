use crate::error::ApiError;
use axum::{extract::FromRequestParts, http::request::Parts};
use school_core::authorization::domain::actor::Actor;
use uuid::Uuid;

pub struct RequestContext {
    pub request_id: String,
    pub correlation_id: String,
    pub idempotency_key: Option<String>,
    pub actor: Option<Actor>,
    pub tenant_id: Uuid,
    #[allow(dead_code)]
    pub school_id: Option<Uuid>,
    #[allow(dead_code)]
    pub ip: Option<String>,
    #[allow(dead_code)]
    pub user_agent: Option<String>,
    #[allow(dead_code)]
    pub locale: Option<String>,
}

impl<S> FromRequestParts<S> for RequestContext
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let request_id = parts
            .headers
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("00000000-0000-0000-0000-000000000000")
            .to_string();

        let correlation_id = parts
            .headers
            .get("x-correlation-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or(&request_id)
            .to_string();

        let idempotency_key = parts
            .headers
            .get("x-idempotency-key")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        let ip = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        let user_agent = parts
            .headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        // Extract Actor from JWT (We'll assume the Auth middleware will insert Actor into extensions)
        let actor = parts.extensions.get::<Actor>().cloned();

        // Extract tenant_id with strict multi-tenant isolation:
        // 1. If an authenticated Actor is present, ALWAYS bind to actor.tenant_id,
        //    UNLESS the actor has a SuperAdmin/PlatformAdmin role and explicitly requests a tenant switch.
        // 2. If no Actor is present (e.g. public endpoint), extract from x-tenant-id header if valid.
        // 3. Otherwise, fallback to Uuid::nil().
        let tenant_id = if let Some(ref a) = actor {
            let is_superadmin = a.roles.iter().any(|r| {
                let name = r.name.to_lowercase();
                name == "superadmin" || name == "platformadmin" || name == "super_admin"
            });

            if is_superadmin {
                parts
                    .headers
                    .get("x-tenant-id")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .unwrap_or(a.tenant_id)
            } else {
                a.tenant_id
            }
        } else if let Some(v) = parts.headers.get("x-tenant-id") {
            v.to_str()
                .ok()
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or(Uuid::nil())
        } else {
            Uuid::nil()
        };

        Ok(RequestContext {
            request_id,
            correlation_id,
            idempotency_key,
            actor,
            tenant_id,
            school_id: None,
            ip,
            user_agent,
            locale: None,
        })
    }
}
