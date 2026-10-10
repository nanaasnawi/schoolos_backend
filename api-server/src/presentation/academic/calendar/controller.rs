use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    bootstrap::ApplicationContext, error::ApiError, extractors::RequestContext,
    response::ApiResponse,
};

#[derive(Debug, Deserialize)]
pub struct CalendarQueryParams {
    #[serde(alias = "academicYear")]
    pub academic_year: Option<String>,
    pub semester: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, utoipa::ToSchema, Clone)]
pub struct CalendarEventItem {
    pub id: Uuid,
    pub academic_year: String,
    pub semester: String,
    pub title: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub category: String,
    pub color: String,
    pub description: Option<String>,
    pub is_effective_learning: bool,
    pub is_national_holiday: bool,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct CalendarMetricsData {
    pub effective_days: i32,
    pub effective_weeks: i32,
    pub holiday_days: i32,
    pub assessment_days: i32,
    pub total_events: i32,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct MonthMebData {
    pub month: i32,
    pub month_name: String,
    pub total_weeks: i32,
    pub effective_weeks: i32,
    pub non_effective_weeks: i32,
    pub effective_hours: i32,
    pub notes: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, Clone)]
pub struct CalendarOverviewResponse {
    pub academic_year: String,
    pub semester: String,
    pub events: Vec<CalendarEventItem>,
    pub metrics: CalendarMetricsData,
    pub meb_breakdown: Vec<MonthMebData>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateCalendarEventPayload {
    #[serde(alias = "academicYear")]
    pub academic_year: Option<String>,
    pub semester: Option<String>,
    pub title: String,
    #[serde(alias = "startDate")]
    pub start_date: NaiveDate,
    #[serde(alias = "endDate")]
    pub end_date: NaiveDate,
    pub category: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    #[serde(alias = "isEffectiveLearning")]
    pub is_effective_learning: Option<bool>,
    #[serde(alias = "isNationalHoliday")]
    pub is_national_holiday: Option<bool>,
    pub action: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteCalendarEventParams {
    pub id: Uuid,
}

pub fn calendar_routes() -> Router<ApplicationContext> {
    Router::new()
        .route("/", get(get_calendar).post(create_or_reset_event).delete(delete_calendar_event))
}

#[utoipa::path(
    get,
    operation_id = "getAcademicCalendar",
    path = "/api/v1/academic/calendar",
    responses(
        (status = 200, description = "Academic Calendar and MEB Analytics", body = ApiResponse<CalendarOverviewResponse>)
    ),
    security(("Bearer" = []))
)]
async fn get_calendar(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<CalendarQueryParams>,
) -> Result<Json<ApiResponse<CalendarOverviewResponse>>, ApiError> {
    let academic_year = params
        .academic_year
        .unwrap_or_else(|| "2026/2027".to_string());
    let semester_filter = params.semester.unwrap_or_else(|| "ALL".to_string());

    // 1. Fetch events from PostgreSQL (scoped to tenant or global template NULL)
    let rows: Vec<CalendarEventItem> = sqlx::query_as::<_, CalendarEventItem>(
        r#"
        SELECT 
            id,
            academic_year,
            semester,
            title,
            start_date,
            end_date,
            category,
            COALESCE(color, '#0284c7') as color,
            description,
            is_effective_learning,
            is_national_holiday
        FROM academic_calendar_events
        WHERE (tenant_id = $1 OR tenant_id IS NULL)
          AND academic_year = $2
          AND deleted_at IS NULL
          AND ($3 = 'ALL' OR semester = $3)
        ORDER BY start_date ASC
        "#,
    )
    .bind(req_ctx.tenant_id)
    .bind(&academic_year)
    .bind(&semester_filter)
    .fetch_all(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    // 2. Business Logic Engine: MEB (Minggu Efektif Belajar) per bulan Kurikulum Merdeka
    let mut meb_breakdown: Vec<MonthMebData> = vec![
        // Semester Ganjil (Juli - Desember)
        MonthMebData {
            month: 7,
            month_name: "Juli 2026".to_string(),
            total_weeks: 4,
            effective_weeks: 2,
            non_effective_weeks: 2,
            effective_hours: 8,
            notes: "MPLS Ramah Anak & Penyesuaian Asesmen Awal Diagnostik".to_string(),
        },
        MonthMebData {
            month: 8,
            month_name: "Agustus 2026".to_string(),
            total_weeks: 5,
            effective_weeks: 4,
            non_effective_weeks: 1,
            effective_hours: 16,
            notes: "HUT RI ke-81 & Peringatan Hari Besar Nasional".to_string(),
        },
        MonthMebData {
            month: 9,
            month_name: "September 2026".to_string(),
            total_weeks: 4,
            effective_weeks: 3,
            non_effective_weeks: 1,
            effective_hours: 12,
            notes: "Sumatif Tengah Semester (STS) Ganjil".to_string(),
        },
        MonthMebData {
            month: 10,
            month_name: "Oktober 2026".to_string(),
            total_weeks: 4,
            effective_weeks: 4,
            non_effective_weeks: 0,
            effective_hours: 16,
            notes: "Pelaksanaan Asesmen Nasional (ANBK) & KBM Efektif".to_string(),
        },
        MonthMebData {
            month: 11,
            month_name: "November 2026".to_string(),
            total_weeks: 4,
            effective_weeks: 4,
            non_effective_weeks: 0,
            effective_hours: 16,
            notes: "Hari Guru Nasional & Persiapan Sumatif Akhir Semester".to_string(),
        },
        MonthMebData {
            month: 12,
            month_name: "Desember 2026".to_string(),
            total_weeks: 5,
            effective_weeks: 1,
            non_effective_weeks: 4,
            effective_hours: 4,
            notes: "SAS Ganjil, Pembagian Rapor & Libur Akhir Semester 1".to_string(),
        },
        // Semester Genap (Januari - Juni)
        MonthMebData {
            month: 1,
            month_name: "Januari 2027".to_string(),
            total_weeks: 4,
            effective_weeks: 4,
            non_effective_weeks: 0,
            effective_hours: 16,
            notes: "Awal Masuk KBM Semester Genap".to_string(),
        },
        MonthMebData {
            month: 2,
            month_name: "Februari 2027".to_string(),
            total_weeks: 4,
            effective_weeks: 4,
            non_effective_weeks: 0,
            effective_hours: 16,
            notes: "Penguatan Gelar Karya P5 & KBM Efektif".to_string(),
        },
        MonthMebData {
            month: 3,
            month_name: "Maret 2027".to_string(),
            total_weeks: 5,
            effective_weeks: 2,
            non_effective_weeks: 3,
            effective_hours: 8,
            notes: "STS Genap, Libur Ramadhan, Ujian Praktik & Idul Fitri".to_string(),
        },
        MonthMebData {
            month: 4,
            month_name: "April 2027".to_string(),
            total_weeks: 4,
            effective_weeks: 3,
            non_effective_weeks: 1,
            effective_hours: 12,
            notes: "Ujian Satuan Pendidikan / PSAJ Kelas Akhir".to_string(),
        },
        MonthMebData {
            month: 5,
            month_name: "Mei 2027".to_string(),
            total_weeks: 5,
            effective_weeks: 3,
            non_effective_weeks: 2,
            effective_hours: 12,
            notes: "Hardiknas, Libur Hari Besar Keagamaan & Persiapan SAT".to_string(),
        },
        MonthMebData {
            month: 6,
            month_name: "Juni 2027".to_string(),
            total_weeks: 4,
            effective_weeks: 1,
            non_effective_weeks: 3,
            effective_hours: 4,
            notes: "SAT Genap, Rapor Kenaikan Kelas & Libur Akhir Tahun Ajaran".to_string(),
        },
    ];

    if semester_filter == "ODD" {
        meb_breakdown.retain(|m| m.month >= 7 && m.month <= 12);
    } else if semester_filter == "EVEN" {
        meb_breakdown.retain(|m| m.month >= 1 && m.month <= 6);
    }

    // 3. Calculate Core Metrics & KPIs
    let total_meb: i32 = meb_breakdown.iter().map(|m| m.effective_weeks).sum();
    let effective_days = total_meb * 5; // 5 hari belajar per minggu

    let mut holiday_days = 0;
    let mut assessment_days = 0;

    for ev in &rows {
        let duration = (ev.end_date - ev.start_date).num_days() + 1;
        let d = duration.max(1) as i32;

        let cat_upper = ev.category.to_uppercase();
        if cat_upper.contains("HOLIDAY") || ev.is_national_holiday {
            holiday_days += d;
        } else if cat_upper == "EXAM" || cat_upper == "ASSESSMENT" {
            assessment_days += d;
        }
    }

    let metrics = CalendarMetricsData {
        effective_days,
        effective_weeks: total_meb,
        holiday_days,
        assessment_days,
        total_events: rows.len() as i32,
    };

    let overview = CalendarOverviewResponse {
        academic_year,
        semester: semester_filter,
        events: rows,
        metrics,
        meb_breakdown,
    };

    Ok(Json(ApiResponse::success(overview, req_ctx.request_id)))
}

#[utoipa::path(
    post,
    operation_id = "createOrResetCalendarEvent",
    path = "/api/v1/academic/calendar",
    request_body = CreateCalendarEventPayload,
    responses(
        (status = 201, description = "Event created or preset reset", body = ApiResponse<CalendarEventItem>)
    ),
    security(("Bearer" = []))
)]
async fn create_or_reset_event(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Json(payload): Json<CreateCalendarEventPayload>,
) -> Result<Json<ApiResponse<Option<CalendarEventItem>>>, ApiError> {
    // Check if reset action requested
    if payload.action.as_deref() == Some("RESET_PRESET") {
        // Soft delete tenant's custom events to restore default baseline view
        sqlx::query(
            r#"
            UPDATE academic_calendar_events
            SET deleted_at = NOW()
            WHERE tenant_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(req_ctx.tenant_id)
        .execute(&ctx.pool)
        .await
        .map_err(|e| {
            ApiError::new(
                school_core::common::error::ApplicationError::Infrastructure(
                    school_core::common::error::InfrastructureError::Database(e),
                ),
                &req_ctx.request_id,
            )
        })?;

        return Ok(Json(ApiResponse::success(None, req_ctx.request_id)));
    }

    let title = payload.title.trim();
    if title.is_empty() {
        return Err(ApiError::new(
            school_core::common::error::ApplicationError::Domain(
                school_core::common::error::DomainError::Validation(
                    "Judul kegiatan agenda tidak boleh kosong".to_string(),
                ),
            ),
            &req_ctx.request_id,
        ));
    }

    let id = Uuid::new_v4();
    let academic_year = payload
        .academic_year
        .unwrap_or_else(|| "2026/2027".to_string());
    let semester = payload.semester.unwrap_or_else(|| "ODD".to_string());
    let category = payload
        .category
        .unwrap_or_else(|| "SCHOOL_EVENT".to_string());
    let color = payload.color.unwrap_or_else(|| "#0284c7".to_string());
    let is_effective = payload.is_effective_learning.unwrap_or(false);
    let is_holiday = payload.is_national_holiday.unwrap_or(false);

    let row = sqlx::query_as::<_, CalendarEventItem>(
        r#"
        INSERT INTO academic_calendar_events (
            id, tenant_id, academic_year, semester, title, start_date, end_date,
            category, color, description, is_effective_learning, is_national_holiday
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING
            id, academic_year, semester, title, start_date, end_date,
            category, color, description, is_effective_learning, is_national_holiday
        "#,
    )
    .bind(id)
    .bind(req_ctx.tenant_id)
    .bind(academic_year)
    .bind(semester)
    .bind(title)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(category)
    .bind(color)
    .bind(payload.description)
    .bind(is_effective)
    .bind(is_holiday)
    .fetch_one(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(Some(row), req_ctx.request_id)))
}

#[utoipa::path(
    delete,
    operation_id = "deleteCalendarEvent",
    path = "/api/v1/academic/calendar",
    responses(
        (status = 200, description = "Event deleted", body = ApiResponse<bool>)
    ),
    security(("Bearer" = []))
)]
async fn delete_calendar_event(
    State(ctx): State<ApplicationContext>,
    req_ctx: RequestContext,
    Query(params): Query<DeleteCalendarEventParams>,
) -> Result<Json<ApiResponse<bool>>, ApiError> {
    sqlx::query(
        r#"
        UPDATE academic_calendar_events
        SET deleted_at = NOW(), deleted_by = $1
        WHERE id = $2 AND (tenant_id = $3 OR tenant_id IS NULL)
        "#,
    )
    .bind(req_ctx.actor.as_ref().map(|a| a.id))
    .bind(params.id)
    .bind(req_ctx.tenant_id)
    .execute(&ctx.pool)
    .await
    .map_err(|e| {
        ApiError::new(
            school_core::common::error::ApplicationError::Infrastructure(
                school_core::common::error::InfrastructureError::Database(e),
            ),
            &req_ctx.request_id,
        )
    })?;

    Ok(Json(ApiResponse::success(true, req_ctx.request_id)))
}
