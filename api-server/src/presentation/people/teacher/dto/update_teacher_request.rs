use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateTeacherRequest {
    pub full_name: Option<String>,
    pub nip: Option<String>,
}
