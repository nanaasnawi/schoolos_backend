use axum::{extract::State, Json};
use chrono::Utc;
use school_core::common::error::ApplicationError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OpaqueQrTokenResponse {
    pub request_id: String,
    pub opaque_token: String,
    pub nonce: String,
    pub token_state: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateQrTokenRequest {
    pub student_name: String,
    pub nisn: String,
    pub nik: String,
    pub mother_name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ClaimQrTokenRequest {
    pub request_id: String,
    pub opaque_token: String,
}

pub async fn create_qr_token(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<CreateQrTokenRequest>,
) -> Result<Json<ApiResponse<OpaqueQrTokenResponse>>, ApiError> {
    let req_id = Uuid::now_v7();
    let opaque_token = format!("opq_{}", Uuid::now_v7());
    let nonce = format!("nonce_{}", Uuid::now_v7());
    let expires_at = Utc::now() + chrono::Duration::minutes(30);

    let student_json = serde_json::json!({
        "student_name": payload.student_name,
        "nisn": payload.nisn,
        "nik": payload.nik,
        "mother_name": payload.mother_name,
    });

    let _ = sqlx::query(
        r#"
        INSERT INTO onboarding_tokens
        (request_id, tenant_id, opaque_token, nonce, student_data, token_state, expires_at)
        VALUES ($1, $2, $3, $4, $5, 'ISSUED', $6)
        "#,
    )
    .bind(req_id)
    .bind(ctx.tenant_id)
    .bind(&opaque_token)
    .bind(&nonce)
    .bind(student_json)
    .bind(expires_at)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            ApplicationError::Internal(format!("Failed to create onboarding token: {}", e)),
            &ctx.request_id,
        )
    })?;

    let res = OpaqueQrTokenResponse {
        request_id: req_id.to_string(),
        opaque_token,
        nonce,
        token_state: "ISSUED".into(),
        expires_at: expires_at.to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(res, ctx.request_id)))
}

pub async fn claim_qr_token(
    ctx: RequestContext,
    state: State<ApplicationContext>,
    Json(payload): Json<ClaimQrTokenRequest>,
) -> Result<Json<ApiResponse<OpaqueQrTokenResponse>>, ApiError> {
    if let Ok(req_uuid) = Uuid::parse_str(&payload.request_id) {
        let _ = sqlx::query(
            r#"
            UPDATE onboarding_tokens
            SET token_state = 'CONSUMED', updated_at = NOW()
            WHERE request_id = $1 AND opaque_token = $2
            "#,
        )
        .bind(req_uuid)
        .bind(&payload.opaque_token)
        .execute(&state.pool)
        .await;
    }

    let res = OpaqueQrTokenResponse {
        request_id: payload.request_id,
        opaque_token: payload.opaque_token,
        nonce: format!("nonce_{}", Uuid::now_v7()),
        token_state: "CONSUMED".into(),
        expires_at: Utc::now().to_rfc3339(),
    };

    Ok(Json(ApiResponse::success(res, ctx.request_id)))
}
