use crate::common::error::InfrastructureError;
use crate::common::models::page::Page;
use crate::people::application::list_students::query::ListStudentsQuery;
use crate::people::domain::read_models::{
    GuardianDetail, StaffDetail, StaffSummary, StudentProfile, StudentSummary, TeacherDetail,
    TeacherSummary,
};
use crate::people::domain::{guardian::Guardian, staff::Staff, student::Student, teacher::Teacher};
use async_trait::async_trait;
use uuid::Uuid;

use crate::common::infrastructure::uow::UnitOfWork;

#[async_trait]
pub trait TeacherRepository: Send + Sync {
    async fn create(
        &self,
        teacher: &Teacher,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;
    async fn update(
        &self,
        teacher: &Teacher,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Teacher>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Teacher>, InfrastructureError>;
}

#[async_trait]
pub trait TeacherQueryRepository: Send + Sync {
    async fn search(
        &self,
        query: crate::people::application::teacher::list::ListTeachersQuery,
    ) -> Result<crate::common::models::page::Page<TeacherSummary>, InfrastructureError>;
    async fn get_detail(&self, id: Uuid) -> Result<Option<TeacherDetail>, InfrastructureError>;
}

#[async_trait]
pub trait StudentRepository: Send + Sync {
    async fn create(
        &self,
        student: &Student,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;
    async fn update(
        &self,
        student: &Student,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Student>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Student>, InfrastructureError>;
}

#[async_trait]
pub trait StudentQueryRepository: Send + Sync {
    async fn search(
        &self,
        query: ListStudentsQuery,
    ) -> Result<Page<StudentSummary>, InfrastructureError>;
    async fn get_profile(&self, id: Uuid) -> Result<Option<StudentProfile>, InfrastructureError>;
}

// ─── Guardian ─────────────────────────────────────────────────────────────────

#[async_trait]
pub trait GuardianRepository: Send + Sync {
    async fn create(
        &self,
        guardian: &Guardian,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;
    async fn update(
        &self,
        guardian: &Guardian,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Guardian>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Guardian>, InfrastructureError>;
}

#[async_trait]
pub trait GuardianQueryRepository: Send + Sync {
    async fn search(
        &self,
        query: crate::people::application::guardian::list::ListGuardiansQuery,
    ) -> Result<Page<GuardianDetail>, InfrastructureError>;
    async fn get_detail(&self, id: Uuid) -> Result<Option<GuardianDetail>, InfrastructureError>;
}

// ─── Staff ────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait StaffRepository: Send + Sync {
    async fn create(
        &self,
        staff: &Staff,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;
    async fn update(
        &self,
        staff: &Staff,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Staff>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Staff>, InfrastructureError>;
}

#[async_trait]
pub trait StaffQueryRepository: Send + Sync {
    async fn search(
        &self,
        query: crate::people::application::staff::list::ListStaffQuery,
    ) -> Result<Page<StaffSummary>, InfrastructureError>;
    async fn get_detail(&self, id: Uuid) -> Result<Option<StaffDetail>, InfrastructureError>;
}
