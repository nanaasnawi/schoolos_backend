use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WeeklyActivityResponse {
    pub student_id: Uuid,
    pub student_name: String,
    pub week_start_date: String,
    pub week_end_date: String,
    pub total_activities: i32,
    pub total_materials: i32,
    pub total_assignments: i32,
    pub total_quizzes: i32,
    pub peak_day: String,
    pub peak_count: i32,
    pub active_days_count: i32,
    pub days: Vec<DailyActivityStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DailyActivityStat {
    pub date: String,             // "2026-10-12"
    pub day_name: String,         // "Senin"
    pub day_short: String,        // "Sen"
    pub day_number: i32,          // 1..7 (1=Senin, ..., 6=Sabtu, 7=Minggu)
    pub is_today: bool,
    pub materials_count: i32,
    pub assignments_count: i32,
    pub quizzes_count: i32,
    pub total_count: i32,
    pub activities: Vec<DailyActivityItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DailyActivityItem {
    pub id: String,
    pub title: String,
    pub category: String,         // "MATERIAL", "ASSIGNMENT", "QUIZ"
    pub category_label: String,   // "Materi Belajar", "Tugas Siswa", "Kuis & CBT"
    pub sub_type: String,         // "DOCUMENT", "VIDEO", "INFOGRAPHIC", "ARTICLE", "HOMEWORK", "EXAM", "UTS", "PTS", etc.
    pub subject_name: Option<String>,
    pub time: String,             // "09:30"
}
