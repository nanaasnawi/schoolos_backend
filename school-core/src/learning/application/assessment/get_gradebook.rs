use crate::common::error::ApplicationError;
use crate::learning::domain::gradebook::GradeBook;
use crate::learning::infrastructure::repository_traits::GradebookRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct GetGradebookQuery {
    pub student_id: Option<Uuid>,
    pub class_id: Uuid,
    pub subject_id: Uuid,
}

pub struct GetGradebookUseCase {
    repo: Arc<dyn GradebookRepository>,
}

impl GetGradebookUseCase {
    pub fn new(repo: Arc<dyn GradebookRepository>) -> Self {
        Self { repo }
    }

    pub async fn execute(
        &self,
        query: GetGradebookQuery,
    ) -> Result<Vec<GradeBook>, ApplicationError> {
        if let Some(student_id) = query.student_id {
            let gb = self
                .repo
                .find_gradebook_by_student_subject(student_id, query.class_id, query.subject_id)
                .await?;
            Ok(gb.into_iter().collect())
        } else {
            Ok(self
                .repo
                .find_gradebooks_by_class(query.class_id, query.subject_id)
                .await?)
        }
    }
}
