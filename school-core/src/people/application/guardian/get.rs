use std::sync::Arc;

use uuid::Uuid;

use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::people::domain::read_models::GuardianDetail;
use crate::people::infrastructure::repository_traits::GuardianQueryRepository;

#[derive(Debug, Clone)]
pub struct GetGuardianQuery {
    pub tenant_id: Uuid,
    pub guardian_id: Uuid,
}

pub struct GetGuardianUseCase {
    guardian_repo: Arc<dyn GuardianQueryRepository>,
}

impl GetGuardianUseCase {
    pub fn new(guardian_repo: Arc<dyn GuardianQueryRepository>) -> Self {
        Self { guardian_repo }
    }

    pub async fn execute(
        &self,
        query: GetGuardianQuery,
    ) -> Result<GuardianDetail, ApplicationError> {
        let detail = self
            .guardian_repo
            .get_detail(query.guardian_id)
            .await?
            .ok_or(ApplicationError::NotFound(
                ErrorCode::GuardianNotFound,
                format!("Guardian not found: {}", query.guardian_id),
            ))?;

        if detail.tenant_id != query.tenant_id {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Guardian does not belong to the tenant".to_string(),
            ));
        }

        Ok(detail)
    }
}
