use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::common::error::ApplicationError;
use crate::common::models::page::{Page, Pagination};
use crate::people::domain::read_models::GuardianDetail;
use crate::people::infrastructure::repository_traits::GuardianQueryRepository;

#[derive(Debug, Clone, Default)]
pub struct GuardianFilter {
    pub search: Option<String>,
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
pub struct ListGuardiansQuery {
    pub tenant_id: Uuid,
    pub filter: GuardianFilter,
    pub pagination: Pagination,
    pub sort: Sort,
}

pub struct ListGuardiansUseCase {
    guardian_repo: Arc<dyn GuardianQueryRepository>,
}

impl ListGuardiansUseCase {
    pub fn new(guardian_repo: Arc<dyn GuardianQueryRepository>) -> Self {
        Self { guardian_repo }
    }

    pub async fn execute(
        &self,
        query: ListGuardiansQuery,
    ) -> Result<Page<GuardianDetail>, ApplicationError> {
        let guardians = self.guardian_repo.search(query).await?;
        Ok(guardians)
    }
}
