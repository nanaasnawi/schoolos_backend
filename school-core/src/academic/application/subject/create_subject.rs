use crate::academic::domain::subject::Subject;
use crate::academic::infrastructure::repository_traits::SubjectRepository;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateSubjectCommand {
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
}

pub struct CreateSubjectUseCase {
    subject_repo: Arc<dyn SubjectRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateSubjectUseCase {
    pub fn new(subject_repo: Arc<dyn SubjectRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            subject_repo,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateSubjectCommand,
    ) -> Result<Subject, ApplicationError> {
        let subject = Subject::new(command.tenant_id, command.code, command.name, &*self.clock);

        self.subject_repo.create(&subject).await?;

        Ok(subject)
    }
}
