use crate::common::domain::aggregate::AggregateRoot;
use crate::common::domain::clock::Clock;
use crate::common::domain::event::DomainEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Staff {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub full_name: String,
    pub nuptk: Option<String>,
    pub jk: Option<String>,
    pub tempat_lahir: Option<String>,
    pub tanggal_lahir: Option<chrono::NaiveDate>,
    pub nip: Option<String>,
    pub status_kepegawaian: Option<String>,
    pub jenis_ptk: Option<String>,
    pub agama: Option<String>,
    pub alamat_jalan: Option<String>,
    pub no_hp: Option<String>,
    pub email: Option<String>,
    pub job_title: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by: Option<Uuid>,

    #[serde(skip)]
    pub domain_events: Vec<Box<dyn DomainEvent>>,
    pub version: i32,
}

impl Clone for Staff {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            tenant_id: self.tenant_id,
            user_id: self.user_id,
            full_name: self.full_name.clone(),
            nuptk: self.nuptk.clone(),
            jk: self.jk.clone(),
            tempat_lahir: self.tempat_lahir.clone(),
            tanggal_lahir: self.tanggal_lahir,
            nip: self.nip.clone(),
            status_kepegawaian: self.status_kepegawaian.clone(),
            jenis_ptk: self.jenis_ptk.clone(),
            agama: self.agama.clone(),
            alamat_jalan: self.alamat_jalan.clone(),
            no_hp: self.no_hp.clone(),
            email: self.email.clone(),
            job_title: self.job_title.clone(),
            is_active: self.is_active,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            deleted_by: self.deleted_by,
            domain_events: Vec::new(),
            version: self.version,
        }
    }
}

impl AggregateRoot for Staff {
    fn id(&self) -> Uuid {
        self.id
    }
    fn version(&self) -> i32 {
        self.version
    }
    fn take_events(&mut self) -> Vec<Box<dyn DomainEvent>> {
        std::mem::take(&mut self.domain_events)
    }
}

impl Staff {
    pub fn new(tenant_id: Uuid, full_name: String, job_title: String, clock: &dyn Clock) -> Self {
        if tenant_id.is_nil() {
            panic!("Staff::new called with nil tenant_id");
        }
        if full_name.trim().is_empty() {
            panic!("Staff::new called with empty full_name");
        }
        if job_title.trim().is_empty() {
            panic!("Staff::new called with empty job_title");
        }
        let now = clock.now();
        Self {
            id: Uuid::now_v7(),
            tenant_id,
            user_id: None,
            full_name,
            nuptk: None,
            jk: None,
            tempat_lahir: None,
            tanggal_lahir: None,
            nip: None,
            status_kepegawaian: None,
            jenis_ptk: None,
            agama: None,
            alamat_jalan: None,
            no_hp: None,
            email: None,
            job_title,
            is_active: true,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by: None,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn rehydrate(
        id: Uuid,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        full_name: String,
        nuptk: Option<String>,
        jk: Option<String>,
        tempat_lahir: Option<String>,
        tanggal_lahir: Option<chrono::NaiveDate>,
        nip: Option<String>,
        status_kepegawaian: Option<String>,
        jenis_ptk: Option<String>,
        agama: Option<String>,
        alamat_jalan: Option<String>,
        no_hp: Option<String>,
        email: Option<String>,
        job_title: String,
        is_active: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
        deleted_by: Option<Uuid>,
    ) -> Self {
        Self {
            id,
            tenant_id,
            user_id,
            full_name,
            nuptk,
            jk,
            tempat_lahir,
            tanggal_lahir,
            nip,
            status_kepegawaian,
            jenis_ptk,
            agama,
            alamat_jalan,
            no_hp,
            email,
            job_title,
            is_active,
            created_at,
            updated_at,
            deleted_at,
            deleted_by,
            domain_events: Vec::new(),
            version: 1,
        }
    }

    pub fn raise_event(&mut self, event: impl DomainEvent + 'static) {
        self.domain_events.push(Box::new(event));
    }

    pub fn link_user(&mut self, user_id: Uuid, clock: &dyn Clock) {
        self.user_id = Some(user_id);
        self.updated_at = clock.now();
    }

    pub fn deactivate(&mut self, clock: &dyn Clock) {
        if self.is_active {
            self.is_active = false;
            self.updated_at = clock.now();
        }
    }

    pub fn activate(&mut self, clock: &dyn Clock) {
        if !self.is_active {
            self.is_active = true;
            self.updated_at = clock.now();
        }
    }
}
