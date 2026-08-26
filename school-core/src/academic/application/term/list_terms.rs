use crate::academic::domain::term::Term;
use crate::academic::infrastructure::repository_traits::TermRepository;
use crate::common::error::ApplicationError;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListTermsQuery {
    pub academic_year_id: Uuid,
}

pub struct ListTermsUseCase {
    term_repo: Arc<dyn TermRepository>,
}

impl ListTermsUseCase {
    pub fn new(term_repo: Arc<dyn TermRepository>) -> Self {
        Self { term_repo }
    }

    pub async fn execute(&self, query: ListTermsQuery) -> Result<Vec<Term>, ApplicationError> {
        let terms = self
            .term_repo
            .find_by_academic_year(query.academic_year_id)
            .await?;
        Ok(terms)
    }
}
