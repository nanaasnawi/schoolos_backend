use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateStudentRequest {
    pub nisn: String,
    pub full_name: String,
    #[schema(value_type = Option<String>)]
    pub nik: Option<String>,
    #[schema(value_type = Option<String>)]
    pub gender: Option<String>,
    #[schema(value_type = Option<String>)]
    pub place_of_birth: Option<String>,
    #[schema(value_type = Option<String>)]
    pub date_of_birth: Option<String>,
    #[schema(value_type = Option<String>)]
    pub religion: Option<String>,
    #[schema(value_type = Option<String>)]
    pub guardian_id: Option<Uuid>,
}
