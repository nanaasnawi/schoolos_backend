use std::sync::Arc;

use uuid::Uuid;

use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::people::domain::read_models::TeacherDetail;
use crate::people::infrastructure::repository_traits::TeacherQueryRepository;

#[derive(Debug, Clone)]
pub struct GetTeacherQuery {
    pub tenant_id: Uuid,
    pub teacher_id: Uuid,
}

pub struct GetTeacherUseCase {
    teacher_repo: Arc<dyn TeacherQueryRepository>,
}

impl GetTeacherUseCase {
    pub fn new(teacher_repo: Arc<dyn TeacherQueryRepository>) -> Self {
        Self { teacher_repo }
    }

    pub async fn execute(&self, query: GetTeacherQuery) -> Result<TeacherDetail, ApplicationError> {
        let detail = self
            .teacher_repo
            .get_detail(query.teacher_id)
            .await?
            .ok_or(ApplicationError::NotFound(
                ErrorCode::TeacherNotFound,
                format!("Teacher not found: {}", query.teacher_id),
            ))?;

        if detail.tenant_id != query.tenant_id {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Teacher does not belong to the tenant".to_string(),
            ));
        }

        Ok(detail)
    }
}
