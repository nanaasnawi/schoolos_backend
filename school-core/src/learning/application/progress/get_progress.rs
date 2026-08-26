use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::student_progress::StudentProgress;
use crate::learning::infrastructure::repository_traits::StudentProgressRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetProgressQuery {
    pub student_id: Uuid,
    pub class_id: Uuid,
    pub subject_id: Uuid,
}

pub struct GetProgressUseCase {
    repo: Arc<dyn StudentProgressRepository>,
}

impl GetProgressUseCase {
    pub fn new(repo: Arc<dyn StudentProgressRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: GetProgressQuery,
    ) -> Result<StudentProgress, ApplicationError> {
        self.repo
            .find_by_student_class_subject(query.student_id, query.class_id, query.subject_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    ErrorCode::ProgressNotFound,
                    format!("Progress for student {} not found", query.student_id),
                )
            })
    }
}
