use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateTeacherRequest {
    pub full_name: String,
    pub nip: Option<String>,
}
