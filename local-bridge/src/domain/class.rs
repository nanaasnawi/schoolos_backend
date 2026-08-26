use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClassSyncRecord {
    pub external_id: String, // rombongan_belajar_id
    pub class_name: String, // nama_rombel
    pub grade_level: String, // tingkat_pendidikan_id
    pub curriculum: String, // kurikulum_id_str
    pub homeroom_teacher_id: Option<String>, // ptk_id of homeroom teacher
}
