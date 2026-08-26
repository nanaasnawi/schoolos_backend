use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const API_VERSION: &str = "v1";

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ApiMeta>,
    pub request_id: String,
    pub timestamp: String,
    pub version: &'static str,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub details: Option<serde_json::Value>,
    pub trace_id: String,
    pub correlation_id: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginationMeta {
    pub page: u64,
    pub page_size: u64,
    pub total_items: u64,
    pub total_pages: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PaginationParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub search: Option<String>,
    pub cursor: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T, request_id: String) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: None,
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: API_VERSION,
        }
    }

    pub fn success_with_meta(data: T, meta: ApiMeta, request_id: String) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            meta: Some(meta),
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: API_VERSION,
        }
    }

    pub fn error(error: ApiErrorDetail, request_id: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            meta: None,
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
            version: API_VERSION,
        }
    }
}
