use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext,
    error::ApiError,
    extractors::RequestContext,
    response::ApiResponse,
};
use school_core::common::error::{ApplicationError, InfrastructureError};

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct GamificationQuery {
    pub student_id: Option<Uuid>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct XpTransactionDto {
    pub id: Uuid,
    pub action_type: String,
    pub reference_id: Option<Uuid>,
    pub xp_amount: i32,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct GamificationStatusResponse {
    pub student_id: Uuid,
    pub total_xp: i64,
    pub level: i32,
    pub streak_days: i32,
    pub last_activity_date: Option<chrono::NaiveDate>,
    pub progress_to_next_level: i32,
    pub next_level_target_xp: i64,
    pub transactions: Vec<XpTransactionDto>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AwardXpPayload {
    pub student_id: Uuid,
    pub action_type: String,
    pub reference_id: Option<Uuid>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AwardXpResponse {
    pub awarded: bool,
    pub already_awarded: bool,
    pub xp_earned: i32,
    pub total_xp: i64,
    pub level: i32,
    pub streak_days: i32,
    pub leveled_up: bool,
    pub message: String,
}

pub fn gamification_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", get(get_student_gamification).post(award_student_xp))
}

fn calculate_level(total_xp: i64) -> i32 {
    let mut lvl = 1;
    while total_xp >= ((lvl as f64).powi(2) * 50.0) as i64 {
        lvl += 1;
    }
    lvl
}

pub async fn get_student_gamification(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(query): Query<GamificationQuery>,
) -> Result<Json<ApiResponse<GamificationStatusResponse>>, ApiError> {
    let target_student_id = match query.student_id {
        Some(sid) => {
            // Cek apakah sid adalah student_id atau user_id
            let resolved = sqlx::query(
                "SELECT id FROM students WHERE (id = $1 OR user_id = $1) AND tenant_id = $2 LIMIT 1"
            )
            .bind(sid)
            .bind(req_ctx.tenant_id)
            .fetch_optional(&ctx.pool)
            .await
            .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;
            
            resolved.map(|r| r.get::<Uuid, _>("id")).unwrap_or(sid)
        }
        None => {
            // Ambil dari authenticated actor jika Siswa
            let actor_id = req_ctx.actor.as_ref().map(|a| a.id).unwrap_or_default();
            let resolved = sqlx::query(
                "SELECT id FROM students WHERE user_id = $1 AND tenant_id = $2 LIMIT 1"
            )
            .bind(actor_id)
            .bind(req_ctx.tenant_id)
            .fetch_optional(&ctx.pool)
            .await
            .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

            resolved.map(|r| r.get::<Uuid, _>("id")).unwrap_or(actor_id)
        }
    };

    let xp_row = sqlx::query(
        "SELECT total_xp, level, streak_days, last_activity_date FROM student_xp WHERE student_id = $1"
    )
    .bind(target_student_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let (total_xp, level, streak_days, last_activity_date) = match xp_row {
        Some(r) => (
            r.get::<i64, _>("total_xp"),
            r.get::<i32, _>("level"),
            r.get::<i32, _>("streak_days"),
            r.try_get::<Option<chrono::NaiveDate>, _>("last_activity_date").ok().flatten(),
        ),
        None => (0, 1, 1, None),
    };

    let current_base = ((level - 1) as f64).powi(2) * 50.0;
    let next_target = (level as f64).powi(2) * 50.0;
    let denom = if (next_target - current_base) > 0.0 { next_target - current_base } else { 1.0 };
    let progress_percent = (((total_xp as f64 - current_base) / denom) * 100.0).clamp(0.0, 100.0) as i32;

    let tx_rows = sqlx::query(
        r#"
        SELECT id, action_type, reference_id, xp_amount, description, created_at
        FROM student_xp_transactions
        WHERE student_id = $1
        ORDER BY created_at DESC
        LIMIT 10
        "#
    )
    .bind(target_student_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap_or_default();

    let transactions: Vec<XpTransactionDto> = tx_rows
        .into_iter()
        .map(|r| XpTransactionDto {
            id: r.get("id"),
            action_type: r.get("action_type"),
            reference_id: r.try_get("reference_id").ok().flatten(),
            xp_amount: r.get("xp_amount"),
            description: r.try_get("description").ok().flatten(),
            created_at: r.get("created_at"),
        })
        .collect();

    Ok(Json(ApiResponse::success(
        GamificationStatusResponse {
            student_id: target_student_id,
            total_xp,
            level,
            streak_days,
            last_activity_date,
            progress_to_next_level: progress_percent,
            next_level_target_xp: next_target as i64,
            transactions,
        },
        req_ctx.request_id,
    )))
}

pub async fn award_student_xp(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<AwardXpPayload>,
) -> Result<Json<ApiResponse<AwardXpResponse>>, ApiError> {
    // Resolve target student id & tenant id
    let resolved_student = sqlx::query(
        "SELECT id, tenant_id FROM students WHERE (id = $1 OR user_id = $1) LIMIT 1"
    )
    .bind(payload.student_id)
    .fetch_optional(&ctx.pool)
    .await
    .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

    let (effective_student_id, effective_tenant_id) = match resolved_student {
        Some(r) => (r.get::<Uuid, _>("id"), r.get::<Uuid, _>("tenant_id")),
        None => (payload.student_id, req_ctx.tenant_id),
    };

    let action_clean = payload.action_type.to_uppercase();
    let xp_amount = match action_clean.as_str() {
        "READ_MATERIAL" => 25,
        "SUBMIT_ASSIGNMENT" => 50,
        "COMPLETE_QUIZ" => 100,
        "COMPLETE_EXAM" => 150,
        _ => 25,
    };

    // 1. Cek idempotency: apakah transaksi sudah ada
    if let Some(ref_id) = payload.reference_id {
        let exists = sqlx::query(
            "SELECT 1 FROM student_xp_transactions WHERE student_id = $1 AND action_type = $2 AND reference_id = $3 LIMIT 1"
        )
        .bind(effective_student_id)
        .bind(&action_clean)
        .bind(ref_id)
        .fetch_optional(&ctx.pool)
        .await
        .map_err(|e| ApiError::new(ApplicationError::Infrastructure(InfrastructureError::Database(e)), &req_ctx.request_id))?;

        if exists.is_some() {
            let cur = sqlx::query("SELECT total_xp, level, streak_days FROM student_xp WHERE student_id = $1")
                .bind(effective_student_id)
                .fetch_optional(&ctx.pool)
                .await
                .ok()
                .flatten();

            let (total_xp, level, streak_days) = match cur {
                Some(r) => (r.get::<i64, _>("total_xp"), r.get::<i32, _>("level"), r.get::<i32, _>("streak_days")),
                None => (0, 1, 1),
            };

            return Ok(Json(ApiResponse::success(
                AwardXpResponse {
                    awarded: false,
                    already_awarded: true,
                    xp_earned: 0,
                    total_xp,
                    level,
                    streak_days,
                    leveled_up: false,
                    message: "XP sudah pernah diraih untuk aktivitas ini.".to_string(),
                },
                req_ctx.request_id,
            )));
        }
    }

    // 2. Insert transaksi
    let _ = sqlx::query(
        r#"
        INSERT INTO student_xp_transactions (id, tenant_id, student_id, action_type, reference_id, xp_amount, description, created_at)
        VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (student_id, action_type, reference_id) DO NOTHING
        "#
    )
    .bind(effective_tenant_id)
    .bind(effective_student_id)
    .bind(&action_clean)
    .bind(payload.reference_id)
    .bind(xp_amount)
    .bind(&payload.description)
    .execute(&ctx.pool)
    .await;

    // 3. Upsert tabel total XP
    let current_xp_row = sqlx::query("SELECT total_xp, level, streak_days, last_activity_date FROM student_xp WHERE student_id = $1")
        .bind(effective_student_id)
        .fetch_optional(&ctx.pool)
        .await
        .ok()
        .flatten();

    let (old_total, old_level, old_streak, last_date) = match current_xp_row {
        Some(r) => (
            r.get::<i64, _>("total_xp"),
            r.get::<i32, _>("level"),
            r.get::<i32, _>("streak_days"),
            r.try_get::<Option<chrono::NaiveDate>, _>("last_activity_date").ok().flatten(),
        ),
        None => (0, 1, 1, None),
    };

    let new_total = old_total + (xp_amount as i64);
    let new_level = calculate_level(new_total);
    let today = chrono::Utc::now().date_naive();
    let new_streak = match last_date {
        Some(d) if d == today => old_streak,
        Some(d) if d == today.pred_opt().unwrap_or(today) => old_streak + 1,
        _ => 1,
    };

    let _ = sqlx::query(
        r#"
        INSERT INTO student_xp (student_id, tenant_id, total_xp, level, streak_days, last_activity_date, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (student_id) DO UPDATE SET
            total_xp = EXCLUDED.total_xp,
            level = EXCLUDED.level,
            streak_days = EXCLUDED.streak_days,
            last_activity_date = EXCLUDED.last_activity_date,
            updated_at = NOW()
        "#
    )
    .bind(effective_student_id)
    .bind(effective_tenant_id)
    .bind(new_total)
    .bind(new_level)
    .bind(new_streak)
    .bind(today)
    .execute(&ctx.pool)
    .await;

    let leveled_up = new_level > old_level;
    let msg = if leveled_up {
        format!("Selamat! Kamu naik ke Level {}! (+{} XP)", new_level, xp_amount)
    } else {
        format!("+{} XP berhasil diraih!", xp_amount)
    };

    Ok(Json(ApiResponse::success(
        AwardXpResponse {
            awarded: true,
            already_awarded: false,
            xp_earned: xp_amount,
            total_xp: new_total,
            level: new_level,
            streak_days: new_streak,
            leveled_up,
            message: msg,
        },
        req_ctx.request_id,
    )))
}
