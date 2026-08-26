use crate::academic::domain::term::Term;
use crate::academic::infrastructure::repository_traits::TermRepository;
use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetTermQuery {
    pub id: Uuid,
}

pub struct GetTermUseCase {
    term_repo: Arc<dyn TermRepository>,
}

impl GetTermUseCase {
    pub fn new(term_repo: Arc<dyn TermRepository>) -> Self {
        Self { term_repo }
    }

    pub async fn execute(&self, query: GetTermQuery) -> Result<Term, ApplicationError> {
        self.term_repo.find_by_id(query.id).await?.ok_or_else(|| {
            ApplicationError::NotFound(
                ErrorCode::ResourceNotFound,
                format!("Term {} not found", query.id),
            )
        })
    }
}
