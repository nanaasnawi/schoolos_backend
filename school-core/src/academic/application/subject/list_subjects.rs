use crate::academic::domain::subject::Subject;
use crate::academic::infrastructure::repository_traits::SubjectRepository;
use crate::common::error::ApplicationError;
use std::sync::Arc;
use uuid::Uuid;

pub struct ListSubjectsQuery {
    pub tenant_id: Uuid,
}

pub struct ListSubjectsUseCase {
    subject_repo: Arc<dyn SubjectRepository>,
}

impl ListSubjectsUseCase {
    pub fn new(subject_repo: Arc<dyn SubjectRepository>) -> Self {
        Self { subject_repo }
    }

    pub async fn execute(
        &self,
        query: ListSubjectsQuery,
    ) -> Result<Vec<Subject>, ApplicationError> {
        Ok(self.subject_repo.find_by_tenant(query.tenant_id).await?)
    }
}
