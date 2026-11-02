use crate::audit::domain::audit_log::{AuditLog, AuthorizationDecision};
use crate::audit::infrastructure::repository_traits::AuditRepository;
use crate::common::domain::clock::Clock;
use crate::common::event_bus::InMemoryEventBus;
use crate::permission::domain::permission_registry::Permission;
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct AuditEventSubscriber;

impl AuditEventSubscriber {
    pub fn start(
        event_bus: Arc<InMemoryEventBus>,
        audit_repo: Arc<dyn AuditRepository>,
        clock: Arc<dyn Clock>,
    ) {
        let mut receiver = event_bus.subscribe();

        tokio::spawn(async move {
            info!(
                component = "audit_engine",
                "Audit Engine started — listening for domain events"
            );

            loop {
                let event = match receiver.recv().await {
                    Ok(e) => e,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(lagged_count = n, "Audit event subscriber lagged, dropped events");
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        info!("Event bus closed, terminating audit event subscriber");
                        break;
                    }
                };

                let metadata = event.metadata();
                let event_id = metadata.event_id;
                let event_name = event.event_name();
                let request_id = metadata.request_id.clone();
                let tenant_id = metadata.tenant_id;
                let source = metadata.source.to_string();

                let decision = AuthorizationDecision {
                    allowed: true,
                    permission: Permission::SystemInternal,
                    policy: Some("DomainEventSubscription".to_string()),
                    reason: Some(match event_name {
                    "AssignmentSubmitted" => "Pengumpulan lembar jawaban tugas oleh siswa".to_string(),
                    "AssignmentCreated" => "Penerbitan tugas pembelajaran baru untuk rombel".to_string(),
                    "LessonPublished" | "MaterialCreated" => "Penerbitan modul atau bahan ajar baru".to_string(),
                    "QuizAttemptStarted" => "Siswa memulai sesi pengerjaan ujian CBT".to_string(),
                    "QuizAttemptFinished" | "QuizCompleted" => "Siswa menyelesaikan pengerjaan kuis CBT".to_string(),
                    "GradeReleased" => "Publikasi nilai hasil penilaian siswa".to_string(),
                    "StudentEnrolled" => "Pendaftaran dan penempatan rombel siswa baru".to_string(),
                    _ => format!("Aktivitas sistem: {} tercatat otomatis dari {}", event_name, source),
                }),
                };

                let audit_log = AuditLog::new(
                    tenant_id,
                    None, // school_id
                    request_id,
                    metadata.actor_id,
                    event_name.to_string(),
                    Some(event_id.to_string()),
                    decision,
                    None, // ip
                    None, // user_agent
                    &*clock,
                );

                match audit_repo.log(&audit_log).await {
                    Ok(_) => {
                        info!(
                            event_id = %event_id,
                            event_name = event_name,
                            tenant_id = %tenant_id,
                            source = source,
                            "Audit log persisted"
                        );
                    }
                    Err(e) => {
                        // Best-effort: audit failure must NOT propagate to the business transaction
                        error!(
                            event_id = %event_id,
                            event_name = event_name,
                            error = ?e,
                            "Failed to persist audit log — best-effort, business transaction unaffected"
                        );
                    }
                }
            }

            info!(component = "audit_engine", "Audit Engine stopped");
        });
    }
}
