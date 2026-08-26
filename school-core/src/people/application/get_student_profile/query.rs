use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GetStudentQuery {
    pub tenant_id: Uuid,
    pub student_id: Uuid,
}
