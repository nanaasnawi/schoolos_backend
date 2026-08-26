use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::learning::domain::gradebook::GradeBook;
use crate::learning::infrastructure::repository_traits::{
    AssessmentRuleRepository, GradebookRepository,
};
use std::sync::Arc;
use uuid::Uuid;

pub struct AssessmentEventListener {
    rule_repo: Arc<dyn AssessmentRuleRepository>,
    gradebook_repo: Arc<dyn GradebookRepository>,
    clock: Arc<dyn Clock>,
}

impl AssessmentEventListener {
    pub fn new(
        rule_repo: Arc<dyn AssessmentRuleRepository>,
        gradebook_repo: Arc<dyn GradebookRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            rule_repo,
            gradebook_repo,
            clock,
        }
    }

    /// Event Handler for SubmissionGraded event from Learning Domain
    pub async fn handle_submission_graded(
        &self,
        tenant_id: Uuid,
        student_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
        assignment_id: Uuid,
        assignment_title: String,
        score: f64,
        max_score: f64,
    ) -> Result<(), ApplicationError> {
        let rule = match self
            .rule_repo
            .find_by_class_subject(class_id, subject_id)
            .await?
        {
            Some(r) => r,
            None => return Ok(()), // No assessment rule configured for this class/subject yet
        };

        let weight = rule
            .components
            .iter()
            .find(|c| {
                c.component_type.eq_ignore_ascii_case("assignment")
                    || c.name.eq_ignore_ascii_case(&assignment_title)
            })
            .map(|c| c.weight_percentage)
            .unwrap_or(0.0);

        let existing_gb = self
            .gradebook_repo
            .find_gradebook_by_student_subject(student_id, class_id, subject_id)
            .await?;

        let mut gradebook = if let Some(gb) = existing_gb {
            gb
        } else {
            GradeBook::new(
                tenant_id,
                student_id,
                class_id,
                subject_id,
                None,
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?
        };

        gradebook
            .record_grade(
                "assignment".to_string(),
                Some(assignment_id),
                assignment_title,
                score,
                max_score,
                weight,
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?;

        self.gradebook_repo.save_gradebook(&gradebook).await?;
        Ok(())
    }

    /// Event Handler for QuizAttemptSubmitted event from Learning Domain
    pub async fn handle_quiz_attempt_submitted(
        &self,
        tenant_id: Uuid,
        student_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
        quiz_id: Uuid,
        quiz_title: String,
        score: f64,
        max_score: f64,
    ) -> Result<(), ApplicationError> {
        let rule = match self
            .rule_repo
            .find_by_class_subject(class_id, subject_id)
            .await?
        {
            Some(r) => r,
            None => return Ok(()),
        };

        let weight = rule
            .components
            .iter()
            .find(|c| {
                c.component_type.eq_ignore_ascii_case("quiz")
                    || c.name.eq_ignore_ascii_case(&quiz_title)
            })
            .map(|c| c.weight_percentage)
            .unwrap_or(0.0);

        let existing_gb = self
            .gradebook_repo
            .find_gradebook_by_student_subject(student_id, class_id, subject_id)
            .await?;

        let mut gradebook = if let Some(gb) = existing_gb {
            gb
        } else {
            GradeBook::new(
                tenant_id,
                student_id,
                class_id,
                subject_id,
                None,
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?
        };

        gradebook
            .record_grade(
                "quiz".to_string(),
                Some(quiz_id),
                quiz_title,
                score,
                max_score,
                weight,
                &*self.clock,
            )
            .map_err(ApplicationError::Domain)?;

        self.gradebook_repo.save_gradebook(&gradebook).await?;
        Ok(())
    }
}
