use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AwardAchievementRequest {
    pub student_id: Uuid,
    pub achievement_id: Uuid,
}
