use thiserror::Error;

use crate::common::error_code::ErrorCode;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Business rule violation: {1} ({0})")]
    BusinessRule(ErrorCode, String),
}

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),

    #[error("Not found: {1} ({0})")]
    NotFound(ErrorCode, String),

    #[error("Unauthorized: {1} ({0})")]
    Unauthorized(ErrorCode, String),

    #[error("Infrastructure error: {0}")]
    Infrastructure(#[from] InfrastructureError),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Error, Debug)]
pub enum InfrastructureError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("System error: {0}")]
    System(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for ApplicationError {
    fn from(err: sqlx::Error) -> Self {
        ApplicationError::Infrastructure(InfrastructureError::Database(err))
    }
}

impl ApplicationError {
    pub fn code(&self) -> ErrorCode {
        match self {
            ApplicationError::Domain(DomainError::Validation(_)) => ErrorCode::ValidationFailed,
            ApplicationError::Domain(DomainError::BusinessRule(code, _)) => code.clone(),
            ApplicationError::NotFound(code, _) => code.clone(),
            ApplicationError::Unauthorized(code, _) => code.clone(),
            ApplicationError::Infrastructure(InfrastructureError::Database(_)) => {
                ErrorCode::DatabaseError
            }
            ApplicationError::Infrastructure(_) => ErrorCode::InternalServerError,
            ApplicationError::Internal(_) => ErrorCode::InternalServerError,
        }
    }
}
