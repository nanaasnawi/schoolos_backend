use std::sync::Arc;

use uuid::Uuid;

use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::people::domain::read_models::StaffDetail;
use crate::people::infrastructure::repository_traits::StaffQueryRepository;

#[derive(Debug, Clone)]
pub struct GetStaffQuery {
    pub tenant_id: Uuid,
    pub staff_id: Uuid,
}

pub struct GetStaffUseCase {
    staff_repo: Arc<dyn StaffQueryRepository>,
}

impl GetStaffUseCase {
    pub fn new(staff_repo: Arc<dyn StaffQueryRepository>) -> Self {
        Self { staff_repo }
    }

    pub async fn execute(&self, query: GetStaffQuery) -> Result<StaffDetail, ApplicationError> {
        let detail =
            self.staff_repo
                .get_detail(query.staff_id)
                .await?
                .ok_or(ApplicationError::NotFound(
                    ErrorCode::StaffNotFound,
                    format!("Staff not found: {}", query.staff_id),
                ))?;

        if detail.tenant_id != query.tenant_id {
            return Err(ApplicationError::Unauthorized(
                ErrorCode::AuthPermissionDenied,
                "Staff does not belong to the tenant".to_string(),
            ));
        }

        Ok(detail)
    }
}
