use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::learning::domain::quiz::Quiz;
use crate::learning::infrastructure::repository_traits::QuizRepository;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateQuizCommand {
    pub tenant_id: Uuid,
    pub lesson_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub duration_minutes: i32,
    pub passing_score: i32,
    pub max_attempts: i32,
    pub shuffle_questions: bool,
    pub shuffle_choices: bool,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
}

pub struct CreateQuizUseCase {
    repo: Arc<dyn QuizRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateQuizUseCase {
    pub fn new(repo: Arc<dyn QuizRepository>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    pub async fn execute(&self, command: CreateQuizCommand) -> Result<Quiz, ApplicationError> {
        let mut quiz = Quiz::new(
            command.tenant_id,
            command.lesson_id,
            command.title,
            command.description,
            command.duration_minutes,
            command.passing_score,
            command.max_attempts,
            command.shuffle_questions,
            command.shuffle_choices,
            command.start_at,
            command.end_at,
            &*self.clock,
        )
        .map_err(ApplicationError::Domain)?;

        self.repo.create(&quiz).await?;

        let _events = quiz.take_events();

        Ok(quiz)
    }
}
