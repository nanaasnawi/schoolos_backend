use super::query::GetStudentQuery;
use crate::common::error::ApplicationError;
use crate::people::domain::read_models::StudentProfile;
use crate::people::infrastructure::repository_traits::StudentQueryRepository;
use std::sync::Arc;

pub struct GetStudentProfileUseCase {
    student_repo: Arc<dyn StudentQueryRepository>,
}

impl GetStudentProfileUseCase {
    pub fn new(student_repo: Arc<dyn StudentQueryRepository>) -> Self {
        Self { student_repo }
    }

    pub async fn execute(
        &self,
        query: GetStudentQuery,
    ) -> Result<StudentProfile, ApplicationError> {
        let profile = self
            .student_repo
            .get_profile(query.student_id)
            .await?
            .ok_or(ApplicationError::NotFound(
                crate::common::error_code::ErrorCode::StudentNotFound,
                format!("Student not found: {}", query.student_id),
            ))?;

        // Basic verification that the student belongs to the tenant
        if profile.student.tenant_id != query.tenant_id {
            return Err(ApplicationError::Unauthorized(
                crate::common::error_code::ErrorCode::AuthPermissionDenied,
                "Student does not belong to the tenant".to_string(),
            ));
        }

        Ok(profile)
    }
}
