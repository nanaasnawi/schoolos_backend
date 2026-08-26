use chrono::{DateTime, Utc};
use school_core::learning::domain::achievement::StudentAchievement;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct StudentAchievementResponse {
    pub id: Uuid,
    pub student_id: Uuid,
    pub achievement_id: Uuid,
    pub earned_at: DateTime<Utc>,
}

impl From<StudentAchievement> for StudentAchievementResponse {
    fn from(sa: StudentAchievement) -> Self {
        Self {
            id: sa.id,
            student_id: sa.student_id,
            achievement_id: sa.achievement_id,
            earned_at: sa.earned_at,
        }
    }
}
