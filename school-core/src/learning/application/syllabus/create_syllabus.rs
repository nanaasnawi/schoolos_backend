use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::learning::domain::syllabus::Syllabus;
use crate::learning::infrastructure::repository_traits::SyllabusRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateSyllabusCommand {
    pub tenant_id: Uuid,
    pub curriculum_id: Uuid,
    pub subject_id: Uuid,
    pub grade_level_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

pub struct CreateSyllabusUseCase {
    syllabus_repo: Arc<dyn SyllabusRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateSyllabusUseCase {
    pub fn new(syllabus_repo: Arc<dyn SyllabusRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            syllabus_repo,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateSyllabusCommand,
    ) -> Result<Syllabus, ApplicationError> {
        let syllabus = Syllabus::new(
            command.tenant_id,
            command.curriculum_id,
            command.subject_id,
            command.grade_level_id,
            command.code,
            command.name,
            command.description,
            &*self.clock,
        );

        self.syllabus_repo.create(&syllabus).await?;

        Ok(syllabus)
    }
}
