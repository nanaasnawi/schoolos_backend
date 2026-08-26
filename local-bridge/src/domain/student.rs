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
/// This is what the Local Bridge communicates to the Cloud Integration Hub.
/// It is agnostic of both the actual Dapodik schema and the School OS internal domain.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StudentSyncRecord {
    pub external_id: String, // Identity in Dapodik (e.g. peserta_didik_id)
    pub full_name: String,
    pub birth_date: Option<NaiveDate>,
    pub gender: Option<Gender>,
    pub external_status: ExternalStudentStatus,
    // Add more fields as needed after schema discovery
}
