use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CreateStudentCommand {
    pub tenant_id: Uuid,
    pub nisn: String,
    pub full_name: String,
    pub nik: Option<String>,
    pub gender: Option<String>,
    pub place_of_birth: Option<String>,
    pub date_of_birth: Option<String>,
    pub religion: Option<String>,
    pub guardian_id: Option<Uuid>,
    pub request_id: Option<String>,
}
