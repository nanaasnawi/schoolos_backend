use crate::integration::contracts::StudentSyncRecord;
use crate::people::domain::student::{Student, StudentStatus};
use crate::common::domain::clock::Clock;
use uuid::Uuid;
use tracing::info;

pub struct SyncDapodikStudentCommand {
    pub tenant_id: Uuid,
    pub sync_record: StudentSyncRecord,
}

pub struct SyncDapodikStudentHandler<'a> {
    pub clock: &'a dyn Clock,
    // Add repository when it exists: pub repo: &'a dyn StudentRepository,
}

impl<'a> SyncDapodikStudentHandler<'a> {
    pub fn new(clock: &'a dyn Clock) -> Self {
        Self { clock }
    }

    pub async fn execute(&self, command: SyncDapodikStudentCommand) -> Result<Student, String> {
        info!(
            "Syncing Dapodik Student: {} (External ID: {})",
            command.sync_record.full_name, command.sync_record.external_id
        );

        // 1. Fetch existing student by NISN or External ID (mocked for now)
        // let existing_student = self.repo.find_by_external_id(&command.sync_record.external_id).await?;
        
        let existing_student: Option<Student> = None; // Mocked

        if let Some(mut student) = existing_student {
            // Update existing student
            student.full_name = command.sync_record.full_name;
            student.updated_at = self.clock.now();
            // Map external status to internal status
            // ...
            
            // self.repo.save(&student).await?;
            Ok(student)
        } else {
            // Create new student
            let mut new_student = Student::register(
                command.tenant_id,
                "0000000000".to_string(), // Dummy NISN if empty
                command.sync_record.full_name,
                None, // nik
                None, // gender
                None, // place_of_birth
                None, // date_of_birth
                None, // religion
                None, // nipd
                None, // alamat_jalan
                None, // no_hp
                None, // email
                None, // Guardian
                self.clock,
            )?;
            
            new_student.status = StudentStatus::Active; // Or map from external status
            
            // self.repo.save(&new_student).await?;
            Ok(new_student)
        }
    }
}
