use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::learning::domain::syllabus_competency::SyllabusCompetency;
use crate::learning::infrastructure::repository_traits::SyllabusRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct AddCompetencyCommand {
    pub tenant_id: Uuid,
    pub syllabus_id: Uuid,
    pub code: String,
    pub competency_type: String,
    pub description: String,
    pub order_index: i32,
}

pub struct AddCompetencyUseCase {
    syllabus_repo: Arc<dyn SyllabusRepository>,
    clock: Arc<dyn Clock>,
}

impl AddCompetencyUseCase {
    pub fn new(syllabus_repo: Arc<dyn SyllabusRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            syllabus_repo,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: AddCompetencyCommand,
    ) -> Result<SyllabusCompetency, ApplicationError> {
        let competency = SyllabusCompetency::new(
            command.tenant_id,
            command.syllabus_id,
            command.code,
            command.competency_type,
            command.description,
            command.order_index,
            &*self.clock,
        );

        self.syllabus_repo.add_competency(&competency).await?;

        Ok(competency)
    }
}
