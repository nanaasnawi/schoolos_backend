use super::models::DapodikWsStudent;
use crate::domain::student::{StudentSyncRecord, Gender, ExternalStudentStatus};
use chrono::NaiveDate;

pub fn map_ws_student_to_sync_record(raw: DapodikWsStudent) -> StudentSyncRecord {
    // Parsing date if valid, otherwise None
    let birth_date = NaiveDate::parse_from_str(&raw.tanggal_lahir, "%Y-%m-%d").ok();
    
    // Determine Gender
    let gender = match raw.jenis_kelamin.as_str() {
        "L" => Some(Gender::Male),
        "P" => Some(Gender::Female),
        _ => None,
    };
    
    // Status Logic - Web service usually returns active students in the current semester unless filtered
    let external_status = ExternalStudentStatus::Active;

    StudentSyncRecord {
        external_id: raw.peserta_didik_id,
        full_name: raw.nama,
        birth_date,
        gender,
        external_status,
    }
}
