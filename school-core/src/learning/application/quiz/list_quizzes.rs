use crate::common::error::ApplicationError;
use crate::learning::domain::quiz::Quiz;
use crate::learning::infrastructure::repository_traits::QuizRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListQuizzesQuery {
    pub tenant_id: Uuid,
}

pub struct ListQuizzesUseCase {
    repo: Arc<dyn QuizRepository>,
}

impl ListQuizzesUseCase {
    pub fn new(repo: Arc<dyn QuizRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, query: ListQuizzesQuery) -> Result<Vec<Quiz>, ApplicationError> {
        Ok(self.repo.find_by_tenant(query.tenant_id).await?)
    }
}
