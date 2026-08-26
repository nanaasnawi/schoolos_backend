use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLessonRequest {
    pub syllabus_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub learning_objectives: Option<String>,
    pub duration_minutes: i32,
    pub order_index: i32,
    pub status: String,
}
