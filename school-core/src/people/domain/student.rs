use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StudentStatus {
    Pending,
    Active,
    Inactive,
    Graduated,
    Transferred,
    Archived,
}

impl StudentStatus {
    /// Stable serialization to database string.
    /// Never use `format!("{:?}")` — enum variant names can change.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Graduated => "graduated",
            Self::Transferred => "transferred",
            Self::Archived => "archived",
        }
    }

    /// Stable deserialization from database string.
    pub fn from_db_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pending" => Self::Pending,
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            "graduated" => Self::Graduated,
            "transferred" | "transferredout" | "transferred_out" | "mutasi_out" => Self::Transferred,
            "archived" => Self::Archived,
            _ => {
                warn!(
                    "Unknown student status from DB: '{}', defaulting to Inactive",
                    s
                );
                Self::Inactive
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Student {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub nisn: String,
    pub full_name: String,
    pub nik: Option<String>,
    pub gender: Option<String>,
    pub place_of_birth: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub religion: Option<String>,
    pub nipd: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub guardian_id: Option<Uuid>,
    pub status: StudentStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn crate::common::domain::event::DomainEvent>>,

    // We should track version for optimistic concurrency, adding it dynamically or using 0 for now
    pub version: i32,
}

impl AggregateRoot for Student {
    fn id(&self) -> Uuid {
        self.id
    }

    fn version(&self) -> i32 {
        self.version
    }

    fn take_events(&mut self) -> Vec<Box<dyn crate::common::domain::event::DomainEvent>> {
        std::mem::take(&mut self.domain_events)
    }
}

impl std::fmt::Debug for Student {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Student")
            .field("id", &self.id)
            .field("tenant_id", &self.tenant_id)
            .field("user_id", &self.user_id)
            .field("nisn", &self.nisn)
            .field("full_name", &self.full_name)
            .field("nik", &self.nik)
            .field("gender", &self.gender)
            .field("place_of_birth", &self.place_of_birth)
            .field("date_of_birth", &self.date_of_birth)
            .field("religion", &self.religion)
            .field("guardian_id", &self.guardian_id)
            .field("status", &self.status)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .field("deleted_at", &self.deleted_at)
            .field("deleted_by", &self.deleted_by)
            // skip domain_events
            .finish()
    }
}

impl Student {
    pub fn register(
        tenant_id: Uuid,
        nisn: String,
        full_name: String,
        nik: Option<String>,
        gender: Option<String>,
        place_of_birth: Option<String>,
        date_of_birth: Option<chrono::NaiveDate>,
        religion: Option<String>,
        nipd: Option<String>,
        alamat_jalan: Option<String>,
        no_hp: Option<String>,
        email: Option<String>,
        guardian_id: Option<Uuid>,
        clock: &dyn Clock,
    ) -> Result<Self, String> {
        if tenant_id.is_nil() {
            return Err("Tenant ID cannot be nil".to_string());
        }
        if full_name.trim().is_empty() {
            return Err("Full name cannot be empty".to_string());
        }
        // Relaxing NISN validation to allow TEMP- prefixes from Dapodik
        // if nisn.len() != 10 || !nisn.chars().all(char::is_numeric) {
        //     return Err("NISN must be exactly 10 digits".to_string());
        // }

        let now = clock.now();
        let student = Self {
            id: Uuid::now_v7(),
            tenant_id,
            user_id: None,
            nisn,
            full_name,
            nik,
            gender,
            place_of_birth,
            date_of_birth,
            religion,
            nipd,
            alamat_jalan,
            no_hp,
            email,
            guardian_id,
            status: StudentStatus::Pending,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        };

        // We can optionally raise an event here, but since the use case already constructs it,
        // we will let the use case call `student.raise_event(...)`.

        Ok(student)
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        nisn: String,
        full_name: String,
        nik: Option<String>,
        gender: Option<String>,
        place_of_birth: Option<String>,
        date_of_birth: Option<chrono::NaiveDate>,
        religion: Option<String>,
        nipd: Option<String>,
        alamat_jalan: Option<String>,
        no_hp: Option<String>,
        email: Option<String>,
        guardian_id: Option<Uuid>,
        status: StudentStatus,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
        deleted_by: Option<Uuid>,
    ) -> Self {
        Self {
            id,
            tenant_id,
            user_id,
            nisn,
            full_name,
            nik,
            gender,
            place_of_birth,
            date_of_birth,
            religion,
            nipd,
            alamat_jalan,
            no_hp,
            email,
            guardian_id,
            status,
            created_at,
            updated_at,
            deleted_at,
            deleted_by,
            domain_events: Vec::new(),
            version: 1, // For now, default to 1, we should probably fetch it from DB later
        }
    }

    pub fn raise_event(&mut self, event: impl crate::common::domain::event::DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }

    pub fn graduate(&mut self, clock: &dyn Clock) {
        if self.status != StudentStatus::Graduated {
            self.status = StudentStatus::Graduated;
            self.updated_at = clock.now();
        }
    }

    pub fn transfer(&mut self, clock: &dyn Clock) {
        if self.status != StudentStatus::Transferred {
            self.status = StudentStatus::Transferred;
            self.updated_at = clock.now();
        }
    }

    pub fn deactivate(&mut self, clock: &dyn Clock) {
        if self.status == StudentStatus::Active {
            self.status = StudentStatus::Inactive;
            self.updated_at = clock.now();
        }
    }

    pub fn link_user(&mut self, user_id: Uuid, clock: &dyn Clock) {
        self.user_id = Some(user_id);
        self.updated_at = clock.now();
    }
}
