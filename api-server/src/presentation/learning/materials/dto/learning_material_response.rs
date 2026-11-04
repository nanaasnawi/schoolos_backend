use chrono::{DateTime, Utc};
use school_core::learning::domain::learning_material::LearningMaterial;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema, Default)]
pub struct LearningMaterialResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lesson_id: Option<Uuid>,
    pub material_type: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub content_type: String,
    pub title: String,
    pub description: Option<String>,
    pub storage_key: Option<String>,
    pub external_url: Option<String>,
    pub order_index: i32,
    pub visibility: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teacher_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teacher_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_at: Option<DateTime<Utc>>,
}

pub fn determine_material_format(
    material_type: &str,
    external_url: Option<&str>,
    description: Option<&str>,
    source_type: Option<&str>,
    library_book_id: Option<Uuid>,
) -> String {
    let m_type = material_type.trim().to_lowercase();
    let ext_url = external_url.unwrap_or("").trim().to_lowercase();

    // 1. YouTube / Educational Video
    if m_type == "video" || ext_url.contains("youtube.com") || ext_url.contains("youtu.be") {
        return "VIDEO".to_string();
    }

    // 2. Official SIBI Book or Uploaded PDF document
    if m_type == "document" || m_type == "pdf" || source_type == Some("LIBRARY") || library_book_id.is_some() {
        return "PDF".to_string();
    }
    if ext_url.ends_with(".pdf") && m_type != "article" && m_type != "image" {
        return "PDF".to_string();
    }

    // 3. Infographic (interactive magazine / image concepts)
    if m_type == "image" || m_type == "infographic" {
        return "INFOGRAPHIC".to_string();
    }

    if let Some(desc) = description {
        let trimmed = desc.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if trimmed.contains(r#""type":"IMAGE""#)
                || trimmed.contains(r#""type":"image""#)
                || trimmed.contains(r#""block_type":"IMAGE""#)
            {
                return "INFOGRAPHIC".to_string();
            }
        }
    }

    // 4. Default: Text Article & Reading
    "ARTICLE".to_string()
}

impl From<LearningMaterial> for LearningMaterialResponse {
    fn from(m: LearningMaterial) -> Self {
        let fmt = determine_material_format(
            &m.material_type,
            m.external_url.as_deref(),
            m.description.as_deref(),
            None,
            None,
        );
        Self {
            id: m.id,
            tenant_id: m.tenant_id,
            lesson_id: m.lesson_id,
            material_type: m.material_type,
            format: fmt.clone(),
            content_type: fmt,
            title: m.title,
            description: m.description,
            storage_key: m.storage_key,
            external_url: m.external_url,
            order_index: m.order_index,
            visibility: m.visibility,
            is_active: m.is_active,
            created_at: m.created_at,
            updated_at: m.updated_at,
            is_completed: None,
            completed_count: None,
            teacher_name: None,
            teacher_id: None,
            class_name: None,
            class_id: None,
            subject_name: None,
            subject_id: None,
            start_page: None,
            end_page: None,
            session_id: None,
            release_at: None,
        }
    }
}
