use crate::store::local_store::LocalStore;
use crate::domain::student::StudentSyncRecord;
use sha2::{Sha256, Digest};
use std::sync::Arc;
use tracing::info;

pub enum ChangeSetType {
    Created,
    Updated,
    Unchanged,
}

pub struct StudentChangeSet {
    pub record: StudentSyncRecord,
    pub change_type: ChangeSetType,
}

pub struct ReconciliationEngine {
    store: Arc<LocalStore>,
}

impl ReconciliationEngine {
    pub fn new(store: Arc<LocalStore>) -> Self {
        Self { store }
    }

    /// Compares a batch of StudentSyncRecords against the local snapshot
    /// and returns only the ones that have changed (Created or Updated).
    pub async fn reconcile_students(&self, records: Vec<StudentSyncRecord>) -> Result<Vec<StudentChangeSet>, String> {
        let mut change_sets = Vec::new();
        let entity_type = "student";

        for record in records {
            // Compute a hash of the canonical contract
            // We use JSON serialization to ensure stable hashing of the contents
            let payload_json = serde_json::to_string(&record).unwrap_or_default();
            
            let mut hasher = Sha256::new();
            hasher.update(payload_json.as_bytes());
            let current_hash = format!("{:x}", hasher.finalize());

            // Check previous hash in LocalStore
            let previous_hash = self.store.get_snapshot_hash(entity_type, &record.external_id)
                .await
                .map_err(|e| e.to_string())?;

            let change_type = match previous_hash {
                Some(prev) if prev == current_hash => ChangeSetType::Unchanged,
                Some(_) => ChangeSetType::Updated,
                None => ChangeSetType::Created,
            };

            // If it's changed, we add it to the change set and update the snapshot
            match change_type {
                ChangeSetType::Unchanged => {
                    // Do nothing, save bandwidth
                }
                _ => {
                    self.store.save_snapshot_hash(entity_type, &record.external_id, &current_hash)
                        .await
                        .map_err(|e| e.to_string())?;

                    change_sets.push(StudentChangeSet {
                        record,
                        change_type,
                    });
                }
            }
        }

        info!("Reconciliation complete. Found {} changes to sync.", change_sets.len());

        Ok(change_sets)
    }
}
