use chrono::{DateTime, Utc};
use school_core::learning::domain::achievement::Achievement;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct AchievementResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub criteria_type: String,
    pub criteria_value: String,
    pub is_published: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Achievement> for AchievementResponse {
    fn from(a: Achievement) -> Self {
        Self {
            id: a.id,
            tenant_id: a.tenant_id,
            title: a.title,
            description: a.description,
            icon: a.icon,
            criteria_type: a.criteria_type,
            criteria_value: a.criteria_value,
            is_published: a.is_published,
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }
}
