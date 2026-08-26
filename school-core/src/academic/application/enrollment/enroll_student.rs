use crate::academic::domain::enrollment::Enrollment;
use crate::academic::infrastructure::repository_traits::{ClassRepository, EnrollmentRepository};
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::people::infrastructure::repository_traits::StudentRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct EnrollStudentCommand {
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub class_id: Uuid,
}

pub struct EnrollStudentUseCase {
    enrollment_repo: Arc<dyn EnrollmentRepository>,
    class_repo: Arc<dyn ClassRepository>,
    student_repo: Arc<dyn StudentRepository>,
    clock: Arc<dyn Clock>,
}

impl EnrollStudentUseCase {
    pub fn new(
        enrollment_repo: Arc<dyn EnrollmentRepository>,
        class_repo: Arc<dyn ClassRepository>,
        student_repo: Arc<dyn StudentRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            enrollment_repo,
            class_repo,
            student_repo,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: EnrollStudentCommand,
    ) -> Result<Enrollment, ApplicationError> {
        let student = self
            .student_repo
            .find_by_id(command.student_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    crate::common::error_code::ErrorCode::StudentNotFound,
                    format!("Student not found: {}", command.student_id),
                )
            })?;

        let class = self
            .class_repo
            .find_by_id(command.class_id)
            .await?
            .ok_or_else(|| {
                ApplicationError::NotFound(
                    crate::common::error_code::ErrorCode::AcademicYearNotFound,
                    format!("Class not found: {}", command.class_id),
                )
            })?; // TODO specific error code for class

        let enrollment = Enrollment::new(
            command.tenant_id,
            student.id,
            class.id,
            class.academic_year_id,
            &*self.clock,
        );

        self.enrollment_repo.create(&enrollment).await?;

        Ok(enrollment)
    }
}
