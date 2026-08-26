use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::common::error::ApplicationError;
use crate::common::models::page::{Page, Pagination};
use crate::people::domain::read_models::TeacherSummary;
use crate::people::infrastructure::repository_traits::TeacherQueryRepository;

#[derive(Debug, Clone, Default)]
pub struct TeacherFilter {
    pub search: Option<String>,
    pub status: Option<String>,
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
pub struct ListTeachersQuery {
    pub tenant_id: Uuid,
    pub filter: TeacherFilter,
    pub pagination: Pagination,
    pub sort: Sort,
}

pub struct ListTeachersUseCase {
    teacher_repo: Arc<dyn TeacherQueryRepository>,
}

impl ListTeachersUseCase {
    pub fn new(teacher_repo: Arc<dyn TeacherQueryRepository>) -> Self {
        Self { teacher_repo }
    }

    pub async fn execute(
        &self,
        query: ListTeachersQuery,
    ) -> Result<Page<TeacherSummary>, ApplicationError> {
        let teachers = self.teacher_repo.search(query).await?;
        Ok(teachers)
    }
}
