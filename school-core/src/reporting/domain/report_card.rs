use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::common::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReportStatus {
  Draft,
  Generated,
  Published,
  Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectGradeEntry {
  pub subject_id: Uuid,
  pub subject_code: String,
  pub subject_name: String,
  pub teacher_name: String,
  pub assignment_score: f64,
  pub quiz_score: f64,
  pub midterm_score: f64,
  pub final_score_exam: f64,
  pub final_score: f64,
  pub letter_grade: String,
  pub is_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttendanceSummary {
  pub total_days: u32,
  pub present_days: u32,
  pub sick_days: u32,
  pub permitted_days: u32,
  pub absent_days: u32,
  pub attendance_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportCard {
  pub id: Uuid,
  pub tenant_id: Uuid,
  pub student_id: Uuid,
  pub student_name: String,
  pub nisn: String,
  pub academic_year_id: Uuid,
  pub academic_year_name: String,
  pub class_id: Uuid,
  pub class_name: String,
  pub homeroom_teacher_name: String,
  pub subject_entries: Vec<SubjectGradeEntry>,
  pub attendance: AttendanceSummary,
  pub gpa: f64,
  pub rank_in_class: Option<u32>,
  pub teacher_notes: Option<String>,
  pub status: ReportStatus,
  pub generated_at: DateTime<Utc>,
  pub published_at: Option<DateTime<Utc>>,
}

impl ReportCard {
  pub fn generate(
    tenant_id: Uuid,
    student_id: Uuid,
    student_name: String,
    nisn: String,
    academic_year_id: Uuid,
    academic_year_name: String,
    class_id: Uuid,
    class_name: String,
    homeroom_teacher_name: String,
    subject_entries: Vec<SubjectGradeEntry>,
    attendance: AttendanceSummary,
    teacher_notes: Option<String>,
  ) -> Result<Self, DomainError> {
    if tenant_id == Uuid::nil() {
      return Err(DomainError::Validation("Tenant ID invalid".to_string()));
    }
    if student_id == Uuid::nil() {
      return Err(DomainError::Validation("Student ID invalid".to_string()));
    }

    let total_final_score: f64 = subject_entries.iter().map(|s| s.final_score).sum();
    let gpa = if !subject_entries.is_empty() {
      total_final_score / (subject_entries.len() as f64)
    } else {
      0.0
    };

    Ok(Self {
      id: Uuid::now_v7(),
      tenant_id,
      student_id,
      student_name,
      nisn,
      academic_year_id,
      academic_year_name,
      class_id,
      class_name,
      homeroom_teacher_name,
      subject_entries,
      attendance,
      gpa,
      rank_in_class: None,
      teacher_notes,
      status: ReportStatus::Generated,
      generated_at: Utc::now(),
      published_at: None,
    })
  }

  pub fn publish(&mut self) -> Result<(), DomainError> {
    if self.status == ReportStatus::Published {
      return Err(DomainError::Validation("Rapor sudah dipublikasikan".to_string()));
    }
    self.status = ReportStatus::Published;
    self.published_at = Some(Utc::now());
    Ok(())
  }
}
