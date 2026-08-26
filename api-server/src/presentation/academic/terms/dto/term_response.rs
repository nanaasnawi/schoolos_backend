use chrono::{DateTime, Utc};
use school_core::academic::domain::term::Term;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct TermResponse {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Term> for TermResponse {
    fn from(term: Term) -> Self {
        Self {
            id: term.id,
            academic_year_id: term.academic_year_id,
            name: term.name,
            is_active: term.is_active,
            created_at: term.created_at,
            updated_at: term.updated_at,
        }
    }
}
