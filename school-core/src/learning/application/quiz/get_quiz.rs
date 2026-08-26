use crate::common::error::ApplicationError;
use crate::common::error_code::ErrorCode;
use crate::learning::domain::quiz::Quiz;
use crate::learning::infrastructure::repository_traits::QuizRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetQuizQuery {
    pub quiz_id: Uuid,
}

pub struct GetQuizUseCase {
    repo: Arc<dyn QuizRepository>,
}

impl GetQuizUseCase {
    pub fn new(repo: Arc<dyn QuizRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(&self, query: GetQuizQuery) -> Result<Quiz, ApplicationError> {
        self.repo.find_by_id(query.quiz_id).await?.ok_or_else(|| {
            ApplicationError::NotFound(
                ErrorCode::QuizNotFound,
                format!("Quiz {} not found", query.quiz_id),
            )
        })
    }
}
