use crate::academic::domain::academic_year::AcademicYear;
use crate::academic::infrastructure::repository_traits::AcademicYearRepository;
use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use chrono::NaiveDate;
use std::sync::Arc;
use uuid::Uuid;

pub struct CreateAcademicYearCommand {
    pub tenant_id: Uuid,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

pub struct CreateAcademicYearUseCase {
    academic_year_repo: Arc<dyn AcademicYearRepository>,
    clock: Arc<dyn Clock>,
}

impl CreateAcademicYearUseCase {
    pub fn new(academic_year_repo: Arc<dyn AcademicYearRepository>, clock: Arc<dyn Clock>) -> Self {
        Self {
            academic_year_repo,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: CreateAcademicYearCommand,
    ) -> Result<AcademicYear, ApplicationError> {
        let year = AcademicYear::new(
            command.tenant_id,
            command.name,
            command.start_date,
            command.end_date,
            &*self.clock,
        );

        self.academic_year_repo.create(&year).await?;

        Ok(year)
    }
}
