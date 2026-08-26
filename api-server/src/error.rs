use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use school_core::common::error::ApplicationError;

pub struct ApiError {
    pub inner: ApplicationError,
    pub request_id: String,
}

impl ApiError {
    pub fn new(inner: ApplicationError, request_id: impl Into<String>) -> Self {
        Self {
            inner,
            request_id: request_id.into(),
        }
    }
}

impl From<ApplicationError> for ApiError {
    fn from(inner: ApplicationError) -> Self {
        Self {
            inner,
            request_id: String::new(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.inner {
            ApplicationError::Domain(_) => StatusCode::BAD_REQUEST,
            ApplicationError::NotFound(_, _) => StatusCode::NOT_FOUND,
            ApplicationError::Unauthorized(_, _) => StatusCode::UNAUTHORIZED,
            ApplicationError::Infrastructure(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApplicationError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let error_code = self.inner.code();
        let error_message = self.inner.to_string();

        let trace_id = Uuid::now_v7().to_string();
        let request_id = if self.request_id.is_empty() {
            trace_id.clone()
        } else {
            self.request_id
        };

        let api_error_detail = crate::response::ApiErrorDetail {
            code: error_code.to_string(),
            message: error_message,
            details: None,
            trace_id: trace_id.clone(),
            correlation_id: request_id.clone(),
        };

        tracing::error!(
            request_id = request_id,
            trace_id = trace_id,
            http_status = status.as_u16(),
            error_code = error_code.to_string(),
            "API error response"
        );

        let body = Json(crate::response::ApiResponse::<()>::error(
            api_error_detail,
            request_id.clone(),
        ));

        let mut response = (status, body).into_response();
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            response
                .headers_mut()
                .insert(header::HeaderName::from_static("x-request-id"), value);
        }
        response
    }
}
