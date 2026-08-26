use serde::{Deserialize, Serialize};
use chrono::NaiveDate;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Gender {
    Male,
    Female,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ExternalStudentStatus {
    Active,
    Graduated,
    TransferredOut,
    DroppedOut,
    Unknown,
}

/// The Canonical Contract for a Student from Dapodik.
/// This matches the payload sent by the Local Bridge Agent.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StudentSyncRecord {
    pub external_id: String, // Identity in Dapodik (e.g. peserta_didik_id)
    pub full_name: String,
    pub birth_date: Option<NaiveDate>,
    pub gender: Option<Gender>,
    pub external_status: ExternalStudentStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeacherSyncRecord {
    pub external_id: String, // ptk_id
    pub full_name: String,
    pub nuptk: Option<String>,
    pub nip: Option<String>,
    pub status_pegawai: String, // e.g. "PNS", "GTY", "Honor"
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClassSyncRecord {
    pub external_id: String, // rombongan_belajar_id
    pub class_name: String, // nama_rombel
    pub grade_level: String, // tingkat_pendidikan_id
    pub curriculum: String, // kurikulum_id_str
    pub homeroom_teacher_id: Option<String>, // ptk_id of homeroom teacher
}

