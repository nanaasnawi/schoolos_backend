use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::common::error::ApplicationError;
use crate::common::models::page::{Page, Pagination};
use crate::people::domain::read_models::StaffSummary;
use crate::people::infrastructure::repository_traits::StaffQueryRepository;

#[derive(Debug, Clone, Default)]
pub struct StaffFilter {
    pub search: Option<String>,
    pub is_active: Option<bool>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub enum SortDirection {
    #[default]
    Desc,
    Asc,
}

#[derive(Debug, Clone)]
pub struct Sort {
    pub field: String,
    pub direction: SortDirection,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            field: "created_at".to_string(),
            direction: SortDirection::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ListStaffQuery {
    pub tenant_id: Uuid,
    pub filter: StaffFilter,
    pub pagination: Pagination,
    pub sort: Sort,
}

pub struct ListStaffUseCase {
    staff_repo: Arc<dyn StaffQueryRepository>,
}

impl ListStaffUseCase {
    pub fn new(staff_repo: Arc<dyn StaffQueryRepository>) -> Self {
        Self { staff_repo }
    }

    pub async fn execute(
        &self,
        query: ListStaffQuery,
    ) -> Result<Page<StaffSummary>, ApplicationError> {
        let staff = self.staff_repo.search(query).await?;
        Ok(staff)
    }
}
