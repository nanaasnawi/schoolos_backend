use crate::academic::domain::{
    academic_year::AcademicYear, class::Class, enrollment::Enrollment, grade_level::GradeLevel,
    subject::Subject, term::Term,
};
use crate::common::error::InfrastructureError;
use crate::common::models::page::Page;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait AcademicYearRepository: Send + Sync {
    async fn create(&self, year: &AcademicYear) -> Result<(), InfrastructureError>;
    async fn find_active(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<AcademicYear>, InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<AcademicYear>, InfrastructureError>;
}

#[async_trait]
pub trait ClassRepository: Send + Sync {
    async fn create(&self, class: &Class) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Class>, InfrastructureError>;
    async fn list(
        &self,
        tenant_id: Uuid,
        academic_year_id: Option<Uuid>,
        page: u64,
        page_size: u64,
    ) -> Result<Page<Class>, InfrastructureError>;
}

#[async_trait]
pub trait GradeLevelRepository: Send + Sync {
    async fn create(&self, grade_level: &GradeLevel) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<GradeLevel>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid)
        -> Result<Vec<GradeLevel>, InfrastructureError>;
    async fn list(
        &self,
        tenant_id: Uuid,
        page: u64,
        page_size: u64,
    ) -> Result<Page<GradeLevel>, InfrastructureError>;
}

#[async_trait]
pub trait EnrollmentRepository: Send + Sync {
    async fn create(&self, enrollment: &Enrollment) -> Result<(), InfrastructureError>;
}

#[async_trait]
pub trait TermRepository: Send + Sync {
    async fn create(&self, term: &Term) -> Result<(), InfrastructureError>;
    async fn update(&self, term: &Term) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Term>, InfrastructureError>;
    async fn find_by_academic_year(
        &self,
        academic_year_id: Uuid,
    ) -> Result<Vec<Term>, InfrastructureError>;
}

#[async_trait]
pub trait SubjectRepository: Send + Sync {
    async fn create(&self, subject: &Subject) -> Result<(), InfrastructureError>;
    async fn update(&self, subject: &Subject) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Subject>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Subject>, InfrastructureError>;
}
