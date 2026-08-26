use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::syllabus::Syllabus;
use crate::learning::infrastructure::repository_traits::SyllabusRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetSyllabusQuery {
    pub tenant_id: Uuid,
    pub syllabus_id: Uuid,
}

pub struct GetSyllabusUseCase {
    syllabus_repo: Arc<dyn SyllabusRepository>,
}

impl GetSyllabusUseCase {
    pub fn new(syllabus_repo: Arc<dyn SyllabusRepository>) -> Self {
        Self { syllabus_repo }
    }

    pub async fn execute(&self, query: GetSyllabusQuery) -> Result<Syllabus, ApplicationError> {
        self.syllabus_repo
            .find_by_id(query.syllabus_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::SyllabusNotFound,
                    format!("Syllabus {} not found", query.syllabus_id),
                )
            })
    }
}
