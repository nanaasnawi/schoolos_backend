use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, FromRow)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub full_name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn new(
        tenant_id: Uuid,
        email: String,
        password_hash: String,
        full_name: String,
        clock: &dyn Clock,
    ) -> Self {
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            email,
            password_hash,
            full_name,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }
}
