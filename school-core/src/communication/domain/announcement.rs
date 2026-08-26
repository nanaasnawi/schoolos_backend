use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::common::error::DomainError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnnouncementScope {
  SchoolWide,
  GradeLevel(u8),
  ClassRoom(Uuid),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
  pub id: Uuid,
  pub tenant_id: Uuid,
  pub author_id: Uuid,
  pub title: String,
  pub content: String,
  pub scope: AnnouncementScope,
  pub is_important: bool,
  pub published_at: DateTime<Utc>,
}

impl Announcement {
  pub fn create(
    tenant_id: Uuid,
    author_id: Uuid,
    title: String,
    content: String,
    scope: AnnouncementScope,
    is_important: bool,
  ) -> Result<Self, DomainError> {
    if tenant_id == Uuid::nil() {
      return Err(DomainError::Validation("Tenant ID invalid".to_string()));
    }
    if title.trim().is_empty() {
      return Err(DomainError::Validation("Judul pengumuman tidak boleh kosong".to_string()));
    }
    if content.trim().is_empty() {
      return Err(DomainError::Validation("Isi pengumuman tidak boleh kosong".to_string()));
    }

    Ok(Self {
      id: Uuid::now_v7(),
      tenant_id,
      author_id,
      title,
      content,
      scope,
      is_important,
      published_at: Utc::now(),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_create_announcement_success() {
    let tenant_id = Uuid::now_v7();
    let author_id = Uuid::now_v7();
    let announcement = Announcement::create(
      tenant_id,
      author_id,
      "Pengumuman Ujian".to_string(),
      "Ujian Tengah Semester akan dimulai hari Senin.".to_string(),
      AnnouncementScope::SchoolWide,
      true,
    ).unwrap();

    assert_eq!(announcement.tenant_id, tenant_id);
    assert_eq!(announcement.author_id, author_id);
    assert_eq!(announcement.is_important, true);
    assert_eq!(announcement.scope, AnnouncementScope::SchoolWide);
  }

  #[test]
  fn test_create_announcement_empty_title_fails() {
    let tenant_id = Uuid::now_v7();
    let author_id = Uuid::now_v7();
    let result = Announcement::create(
      tenant_id,
      author_id,
      "".to_string(),
      "Isi pengumuman".to_string(),
      AnnouncementScope::SchoolWide,
      false,
    );

    assert!(result.is_err());
  }
}
