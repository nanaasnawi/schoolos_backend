use crate::common::domain::event::DomainEvent;
use crate::common::event_bus::InMemoryEventBus;
use crate::notification::domain::notification::Notification;
use crate::notification::domain::notification_channel::NotificationChannel;
use crate::notification::infrastructure::repository_traits::NotificationRepository;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

// ── Template Registry ─────────────────────────────────────────────────────

struct NotificationTemplate {
    pub category: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

fn get_template(event_name: &str) -> Option<&'static NotificationTemplate> {
    Some(match event_name {
        "learning.assignment.published" => &NotificationTemplate {
            category: "assignment_published",
            title: "New Assignment",
            body: "A new assignment has been published",
        },
        "learning.assignment.submitted" => &NotificationTemplate {
            category: "assignment_submitted",
            title: "Assignment Submitted",
            body: "An assignment has been submitted",
        },
        "learning.assignment.grade_released" => &NotificationTemplate {
            category: "grade_released",
            title: "Grade Released",
            body: "Your assignment grade has been released",
        },
        "learning.quiz.published" => &NotificationTemplate {
            category: "quiz_published",
            title: "New Quiz",
            body: "A new quiz is now available",
        },
        "learning.quiz.attempt_completed" => &NotificationTemplate {
            category: "quiz_completed",
            title: "Quiz Completed",
            body: "You have completed a quiz",
        },
        "learning.quiz.graded" => &NotificationTemplate {
            category: "quiz_graded",
            title: "Quiz Graded",
            body: "Your quiz has been graded",
        },
        "learning.assessment.grade_calculated" => &NotificationTemplate {
            category: "grade_calculated",
            title: "Grade Calculated",
            body: "A new grade has been calculated",
        },
        "learning.session.lesson_started" => &NotificationTemplate {
            category: "lesson_started",
            title: "Lesson Started",
            body: "A lesson has started",
        },
        "learning.session.lesson_completed" => &NotificationTemplate {
            category: "lesson_completed",
            title: "Lesson Completed",
            body: "A lesson has been completed",
        },
        "learning.progress.updated" => &NotificationTemplate {
            category: "progress_updated",
            title: "Progress Updated",
            body: "Your learning progress has been updated",
        },
        "learning.achievement.earned" => &NotificationTemplate {
            category: "achievement_earned",
            title: "Achievement Earned!",
            body: "Congratulations! You earned a new achievement!",
        },
        _ => return None,
    })
}

// ── Helper event payload extractors ────────────────────────────────────────

#[derive(Deserialize)]
struct EventWithStudentId {
    student_id: Option<Uuid>,
}

#[derive(Deserialize)]
struct EventWithUserId {
    user_id: Option<Uuid>,
}

fn extract_user_id(event: &Arc<dyn DomainEvent>) -> Option<Uuid> {
    // Try student_id first for known events
    if let Ok(val) = serde_json::from_value::<EventWithStudentId>(event.to_json_value()) {
        if let Some(sid) = val.student_id {
            return Some(sid);
        }
    }

    // Try user_id
    if let Ok(val) = serde_json::from_value::<EventWithUserId>(event.to_json_value()) {
        if let Some(uid) = val.user_id {
            return Some(uid);
        }
    }

    // Fall back to actor_id from metadata
    event.metadata().actor_id
}

// ── Subscriber ─────────────────────────────────────────────────────────────

pub struct NotificationEventSubscriber;

impl NotificationEventSubscriber {
    pub fn start(
        event_bus: Arc<InMemoryEventBus>,
        notification_repo: Arc<dyn NotificationRepository>,
    ) {
        let mut receiver = event_bus.subscribe();

        tokio::spawn(async move {
            info!(
                component = "notification_engine",
                "Notification Engine started — listening for domain events"
            );

            while let Ok(event) = receiver.recv().await {
                let event_name = event.event_name();

                let template = match get_template(event_name) {
                    Some(t) => t,
                    None => continue,
                };

                let user_id = match extract_user_id(&event) {
                    Some(uid) => uid,
                    None => {
                        warn!(
                            event_name = event_name,
                            "Notification: no user_id found in event, skipping"
                        );
                        continue;
                    }
                };

                let tenant_id = event.metadata().tenant_id;

                let notification = Notification::new(
                    tenant_id,
                    user_id,
                    template.title.to_string(),
                    template.body.to_string(),
                    template.category.to_string(),
                    NotificationChannel::InApp,
                    Some(event_name.to_string()),
                    None,
                );

                match notification_repo.create(&notification).await {
                    Ok(_) => {
                        info!(
                            event_name = event_name,
                            user_id = %user_id,
                            notification_id = %notification.id,
                            "In-app notification created"
                        );
                    }
                    Err(e) => {
                        error!(
                            event_name = event_name,
                            user_id = %user_id,
                            error = ?e,
                            "Failed to persist notification — best-effort, event unaffected"
                        );
                    }
                }
            }

            info!(
                component = "notification_engine",
                "Notification Engine stopped"
            );
        });
    }
}
