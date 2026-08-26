use crate::academic::domain::term::Term;
use crate::academic::infrastructure::repository_traits::TermRepository;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateTermCommand {
    pub academic_year_id: Uuid,
    pub name: String,
}

pub struct CreateTermUseCase {
    term_repo: Arc<dyn TermRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateTermUseCase {
    pub fn new(term_repo: Arc<dyn TermRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { term_repo, clock }
    }

    pub async fn execute(&self, command: CreateTermCommand) -> Result<Term, ApplicationError> {
        let term = Term::new(command.academic_year_id, command.name, &*self.clock);

        self.term_repo.create(&term).await?;

        Ok(term)
    }
}
