use axum::body::to_bytes;
use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::bootstrap::ApplicationContext;

pub async fn idempotency_middleware(
    State(state): State<ApplicationContext>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if req.method() != axum::http::Method::POST {
        return Ok(next.run(req).await);
    }

    let headers = req.headers().clone();
    let idempotency_key = headers
        .get("x-idempotency-key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    let tenant_id_str = headers.get("x-tenant-id").and_then(|h| h.to_str().ok());

    let tenant_id = if let Some(t) = tenant_id_str {
        Uuid::parse_str(t).unwrap_or_default()
    } else {
        Uuid::default()
    };

    if idempotency_key.is_none() {
        return Ok(next.run(req).await);
    }
    let idempotency_key = idempotency_key.unwrap();

    let (parts, body) = req.into_parts();

    let bytes = match to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
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

        if let Ok(resp_bytes) = to_bytes(resp_body, usize::MAX).await {
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
