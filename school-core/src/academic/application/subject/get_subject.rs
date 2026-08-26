use crate::academic::domain::subject::Subject;
use crate::academic::infrastructure::repository_traits::SubjectRepository;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetSubjectQuery {
    pub tenant_id: Uuid,
    pub subject_id: Uuid,
}

pub struct GetSubjectUseCase {
    subject_repo: Arc<dyn SubjectRepository>,
}

impl GetSubjectUseCase {
    pub fn new(subject_repo: Arc<dyn SubjectRepository>) -> Self {
        Self { subject_repo }
    }

    pub async fn execute(&self, query: GetSubjectQuery) -> Result<Subject, ApplicationError> {
        self.subject_repo
            .find_by_id(query.subject_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::SubjectNotFound,
                    format!("Subject {} not found", query.subject_id),
                )
            })
    }
}
