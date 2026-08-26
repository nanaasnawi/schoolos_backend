use crate::common::error::ApplicationError;
use crate::learning::domain::syllabus_competency::SyllabusCompetency;
use crate::learning::infrastructure::repository_traits::SyllabusRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListCompetenciesQuery {
    pub syllabus_id: Uuid,
}

pub struct ListCompetenciesUseCase {
    syllabus_repo: Arc<dyn SyllabusRepository>,
}

impl ListCompetenciesUseCase {
    pub fn new(syllabus_repo: Arc<dyn SyllabusRepository>) -> Self {
        Self { syllabus_repo }
    }

    pub async fn execute(
        &self,
        query: ListCompetenciesQuery,
    ) -> Result<Vec<SyllabusCompetency>, ApplicationError> {
        Ok(self
            .syllabus_repo
            .find_competencies(query.syllabus_id)
            .await?)
    }
}
