use axum::body::to_bytes;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;

/// Ukuran maksimal body yang di-hash untuk idempotency (1 MB).
/// Body yang lebih besar dari batas ini tidak di-hash dan tidak disimpan,
/// tapi request tetap diproses secara normal.
const MAX_BODY_HASH_SIZE: usize = 1_048_576;

pub async fn idempotency_middleware(
    State(state): State<ApplicationContext>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Hanya aktif untuk POST requests
    if req.method() != Method::POST {
        return Ok(next.run(req).await);
    }

    // Skip middleware untuk multipart/form-data (file upload) — tidak perlu idempotency
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if content_type.starts_with("multipart/") {
        return Ok(next.run(req).await);
    }

    let headers = req.headers().clone();
    let idempotency_key = headers
        .get("x-idempotency-key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    if idempotency_key.is_none() {
        return Ok(next.run(req).await);
    }
    let idempotency_key = idempotency_key.unwrap();

    let actor_tenant_id = req
        .extensions()
        .get::<school_core::common::security::Actor>()
        .map(|a| a.tenant_id)
        .filter(|tid| !tid.is_nil())
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|auth| auth.strip_prefix("Bearer "))
                .and_then(|token| {
                    jsonwebtoken::decode::<
                        school_core::identity::application::auth::authenticate_user::Claims,
                    >(
                        token.trim(),
                        &jsonwebtoken::DecodingKey::from_secret(state.jwt_secret.as_bytes()),
                        &jsonwebtoken::Validation::default(),
                    )
                    .ok()
                })
                .and_then(|td| Uuid::parse_str(&td.claims.tenant_id).ok())
                .filter(|tid| !tid.is_nil())
        });

    let tenant_id = actor_tenant_id.unwrap_or_else(|| {
        headers
            .get("x-tenant-id")
            .and_then(|h| h.to_str().ok())
            .and_then(|t| Uuid::parse_str(t).ok())
            .unwrap_or_default()
    });

    let (parts, body) = req.into_parts();

    // Baca body dengan batas ukuran — kalau lebih besar dari MAX_BODY_HASH_SIZE,
    // body tidak bisa di-hash dan idempotency key tidak disimpan.
    let bytes = match to_bytes(body, MAX_BODY_HASH_SIZE).await {
        Ok(b) => b,
        Err(_) => {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
    };

    let hash = hex::encode(Sha256::digest(&bytes));

    let existing: Option<(i32, Value, String)> = sqlx::query_as(
        "SELECT response_status, response_body, request_hash FROM idempotency_keys WHERE idempotency_key = $1 AND tenant_id = $2",
    )
    .bind(&idempotency_key)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    if let Some((status, response_body, request_hash)) = existing {
        if request_hash != hash {
            return Err(StatusCode::CONFLICT);
        }

        let status_code =
            StatusCode::from_u16(status as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let resp = (status_code, axum::Json(response_body)).into_response();
        return Ok(resp);
    }

    let req = Request::from_parts(parts, Body::from(bytes));
    let mut response = next.run(req).await;
    let status = response.status().as_u16();

    if (200..300).contains(&status) {
        let (resp_parts, resp_body) = response.into_parts();

        if let Ok(resp_bytes) = to_bytes(resp_body, MAX_BODY_HASH_SIZE).await {
            if let Ok(resp_json) = serde_json::from_slice::<Value>(&resp_bytes) {
                let _ = sqlx::query(
                    "INSERT INTO idempotency_keys (idempotency_key, tenant_id, request_hash, response_status, response_body) VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
                )
                .bind(&idempotency_key)
                .bind(tenant_id)
                .bind(hash)
                .bind(status as i32)
                .bind(resp_json)
                .execute(&state.pool)
                .await;
            }
            response = Response::from_parts(resp_parts, Body::from(resp_bytes));
        } else {
            response = Response::from_parts(resp_parts, Body::empty());
        }
    }

    Ok(response)
}
