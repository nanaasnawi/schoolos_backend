use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateStudentRequest {
    #[schema(value_type = Option<String>)]
    pub nisn: Option<String>,
    #[schema(value_type = Option<String>)]
    pub full_name: Option<String>,
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
}
