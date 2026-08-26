use crate::common::models::page::Pagination;
use crate::people::domain::student::StudentStatus;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Filter criteria for listing students.
/// Designed for expansion — add new optional fields freely without breaking existing callers.
#[derive(Debug, Clone, Default)]
pub struct StudentFilter {
    pub search: Option<String>,
    pub grade_level_id: Option<Uuid>,
    pub class_id: Option<Uuid>,
    pub status: Option<StudentStatus>,
    /// Filter by academic year (e.g., for enrollment-based queries)
    pub academic_year_id: Option<Uuid>,
    /// Return only students created after this timestamp
    pub created_after: Option<DateTime<Utc>>,
    /// Return only students created before this timestamp
    pub created_before: Option<DateTime<Utc>>,
    /// Return only students updated after this timestamp
    pub updated_after: Option<DateTime<Utc>>,
    /// Return only students updated before this timestamp
    pub updated_before: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
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
pub struct ListStudentsQuery {
    pub tenant_id: Uuid,
    pub filter: StudentFilter,
    pub pagination: Pagination,
    pub sort: Sort,
}
