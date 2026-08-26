use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct UpdateStudentCommand {
    pub tenant_id: Uuid,
    pub student_id: Uuid,
    pub nisn: Option<String>,
    pub full_name: Option<String>,
    pub nik: Option<String>,
    pub gender: Option<String>,
    pub place_of_birth: Option<String>,
    pub date_of_birth: Option<String>,
    pub religion: Option<String>,
    pub request_id: Option<String>,
}
