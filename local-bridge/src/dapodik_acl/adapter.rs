use async_trait::async_trait;

// Placeholder for Canonical Student until domain layer is built
use crate::domain::student::StudentSyncRecord;

#[async_trait]
pub trait DapodikAdapter {
    /// Connects to the underlying database/API and returns the schema fingerprint
    async fn connect_and_fingerprint(&self) -> Result<String, Box<dyn std::error::Error>>;
    
    /// Retrieves a batch of students based on a sync cursor
    async fn get_students(
        &self, 
        cursor: i64, 
        limit: i32
    ) -> Result<Vec<StudentSyncRecord>, Box<dyn std::error::Error>>;

    /// Retrieves a batch of teachers (PTK)
    async fn get_teachers(
        &self, 
        cursor: i64, 
        limit: i32
    ) -> Result<Vec<crate::domain::teacher::TeacherSyncRecord>, Box<dyn std::error::Error>>;

    /// Retrieves a batch of classes (Rombel)
    async fn get_classes(
        &self, 
        cursor: i64, 
        limit: i32
    ) -> Result<Vec<crate::domain::class::ClassSyncRecord>, Box<dyn std::error::Error>>;
}
