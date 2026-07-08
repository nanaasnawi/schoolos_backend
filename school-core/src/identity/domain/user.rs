use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    SuperAdmin,
    Principal,
    Admin,
    Teacher,
    Student,
    Parent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub tenant_id: Uuid, // Belongs to a tenant (School)
    pub email: String,
    pub password_hash: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub fn new(
        tenant_id: Uuid,
        email: impl Into<String>,
        role: Role,
        password_hash: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            email: email.into(),
            password_hash: password_hash.into(),
            role,
            created_at: Utc::now(),
        }
    }
}
