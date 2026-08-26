use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeacherSyncRecord {
    pub external_id: String, // ptk_id
    pub full_name: String,
    pub nuptk: Option<String>,
    pub nip: Option<String>,
    pub status_pegawai: String, // e.g. "PNS", "GTY", "Honor"
    pub is_active: bool,
}
