use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateLessonRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub learning_objectives: Option<String>,
    pub duration_minutes: Option<i32>,
}
