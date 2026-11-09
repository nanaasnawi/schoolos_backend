use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StudyTrackerResponse {
    pub student_id: Uuid,
    pub student_name: String,
    pub history: TrackerCategory,
    pub missed: TrackerCategory,
    pub in_progress: TrackerCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrackerCategory {
    pub key: String,                  // "history", "missed", "in_progress"
    pub title: String,                // "Riwayat Belajar", "Terlewat", "Belum Selesai"
    pub description: String,          // Friendly student description
    pub total_count: i32,
    pub items: Vec<TrackerItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrackerItem {
    pub id: String,
    pub title: String,
    pub item_type: String,            // "MATERIAL", "ASSIGNMENT", "QUIZ", "SESSION"
    pub sub_type: Option<String>,     // "VIDEO", "DOCUMENT", "TUGAS", "CBT", etc.
    pub subject_name: String,
    pub progress_percent: Option<i32>,
    pub score: Option<f64>,
    pub max_score: Option<f64>,
    pub timestamp_label: String,      // e.g. "Tuntas dibaca • 8 Nov", "Tenggat: 5 Nov (Terlewat)"
    pub date_iso: Option<String>,
    pub status_badge: String,         // "Selesai", "Belum Dibuka", "Lewat Tenggat", "Perlu Revisi", etc.
    pub badge_variant: String,        // "success", "warning", "danger", "info", "neutral"
    pub reason_note: Option<String>,  // Friendly Indonesian note for student
    pub action_url: String,           // Direct Next.js student route / Android deep link
}

