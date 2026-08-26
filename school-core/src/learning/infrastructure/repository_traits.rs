use crate::common::error::InfrastructureError;
use crate::learning::domain::achievement::{Achievement, StudentAchievement};
use crate::learning::domain::assessment_rule::AssessmentComponent;
use crate::learning::domain::assessment_rule::AssessmentRule;
use crate::learning::domain::assignment::Assignment;
use crate::learning::domain::assignment_submission::AssignmentSubmission;
use crate::learning::domain::attempt_answer::AttemptAnswer;
use crate::learning::domain::classroom_feed::FeedItem;
use crate::learning::domain::curriculum::Curriculum;
use crate::learning::domain::gradebook_entry::GradebookEntry;
use crate::learning::domain::learning_material::LearningMaterial;
use crate::learning::domain::learning_session::LearningSession;
use crate::learning::domain::lesson::Lesson;
use crate::learning::domain::lesson_plan::LessonPlan;
use crate::learning::domain::quiz::Quiz;
use crate::learning::domain::quiz_attempt::QuizAttempt;
use crate::learning::domain::quiz_choice::QuizChoice;
use crate::learning::domain::quiz_question::QuizQuestion;
use crate::learning::domain::session_attendance::SessionAttendance;
use crate::learning::domain::student_progress::StudentProgress;
use crate::learning::domain::syllabus::Syllabus;
use crate::learning::domain::syllabus_competency::SyllabusCompetency;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait AssignmentRepository: Send + Sync {
    async fn create(&self, assignment: &Assignment) -> Result<(), InfrastructureError>;
    async fn update(&self, assignment: &Assignment) -> Result<(), InfrastructureError>;
    async fn delete(&self, id: Uuid, deleted_by: Uuid) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Assignment>, InfrastructureError>;
    async fn find_by_lesson(&self, lesson_id: Uuid)
        -> Result<Vec<Assignment>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid)
        -> Result<Vec<Assignment>, InfrastructureError>;
    async fn list_published(&self, tenant_id: Uuid)
        -> Result<Vec<Assignment>, InfrastructureError>;
    async fn count_by_lesson(&self, lesson_id: Uuid) -> Result<i64, InfrastructureError>;

    async fn submit(&self, submission: &AssignmentSubmission) -> Result<(), InfrastructureError>;
    async fn add_attempt(
        &self,
        attempt: &crate::learning::domain::assignment_submission::SubmissionAttempt,
    ) -> Result<(), InfrastructureError>;
    async fn update_submission(
        &self,
        submission: &AssignmentSubmission,
    ) -> Result<(), InfrastructureError>;
    async fn find_submission_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<AssignmentSubmission>, InfrastructureError>;
    async fn find_submissions(
        &self,
        assignment_id: Uuid,
    ) -> Result<Vec<AssignmentSubmission>, InfrastructureError>;
    async fn find_attempts(
        &self,
        submission_id: Uuid,
    ) -> Result<
        Vec<crate::learning::domain::assignment_submission::SubmissionAttempt>,
        InfrastructureError,
    >;
    async fn list_pending_grading(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<AssignmentSubmission>, InfrastructureError>;
}

#[async_trait]
pub trait CurriculumRepository: Send + Sync {
    async fn create(&self, curriculum: &Curriculum) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Curriculum>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid)
        -> Result<Vec<Curriculum>, InfrastructureError>;
}

#[async_trait]
pub trait SyllabusRepository: Send + Sync {
    async fn create(&self, syllabus: &Syllabus) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Syllabus>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Syllabus>, InfrastructureError>;
    async fn add_competency(
        &self,
        competency: &SyllabusCompetency,
    ) -> Result<(), InfrastructureError>;
    async fn find_competencies(
        &self,
        syllabus_id: Uuid,
    ) -> Result<Vec<SyllabusCompetency>, InfrastructureError>;
}

#[async_trait]
pub trait LearningMaterialRepository: Send + Sync {
    async fn create(&self, material: &LearningMaterial) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<LearningMaterial>, InfrastructureError>;
    async fn find_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<LearningMaterial>, InfrastructureError>;
    async fn update(&self, material: &LearningMaterial) -> Result<(), InfrastructureError>;
    async fn delete(&self, id: Uuid, deleted_by: Uuid) -> Result<(), InfrastructureError>;
}

#[async_trait]
pub trait LessonRepository: Send + Sync {
    async fn create(&self, lesson: &Lesson) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Lesson>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Lesson>, InfrastructureError>;
    async fn update(&self, lesson: &Lesson) -> Result<(), InfrastructureError>;
    async fn delete(&self, id: Uuid, deleted_by: Uuid) -> Result<(), InfrastructureError>;
}

#[async_trait]
pub trait LessonPlanRepository: Send + Sync {
    async fn create(&self, plan: &LessonPlan) -> Result<(), InfrastructureError>;
    async fn find_by_lesson_id(
        &self,
        lesson_id: Uuid,
    ) -> Result<Option<LessonPlan>, InfrastructureError>;
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, session: &LearningSession) -> Result<(), InfrastructureError>;
    async fn update(&self, session: &LearningSession) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<LearningSession>, InfrastructureError>;
    async fn find_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<LearningSession>, InfrastructureError>;
    async fn record_attendance(
        &self,
        attendance: &SessionAttendance,
    ) -> Result<(), InfrastructureError>;
    async fn find_attendance(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<SessionAttendance>, InfrastructureError>;
    async fn find_by_class(
        &self,
        class_id: Uuid,
    ) -> Result<Vec<LearningSession>, InfrastructureError>;
    async fn find_attendance_by_student(
        &self,
        student_id: Uuid,
        class_id: Uuid,
    ) -> Result<Vec<SessionAttendance>, InfrastructureError>;
}

#[async_trait]
pub trait QuizRepository: Send + Sync {
    // Quiz CRUD
    async fn create(&self, quiz: &Quiz) -> Result<(), InfrastructureError>;
    async fn update(&self, quiz: &Quiz) -> Result<(), InfrastructureError>;
    async fn delete(&self, id: Uuid, deleted_by: Uuid) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Quiz>, InfrastructureError>;
    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Quiz>, InfrastructureError>;

    // Questions & Choices
    async fn save_question(&self, question: &QuizQuestion) -> Result<(), InfrastructureError>;
    async fn save_choice(&self, choice: &QuizChoice) -> Result<(), InfrastructureError>;
    async fn find_questions(&self, quiz_id: Uuid)
        -> Result<Vec<QuizQuestion>, InfrastructureError>;

    // Attempts
    async fn create_attempt(&self, attempt: &QuizAttempt) -> Result<(), InfrastructureError>;
    async fn update_attempt(&self, attempt: &QuizAttempt) -> Result<(), InfrastructureError>;
    async fn find_attempt_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<QuizAttempt>, InfrastructureError>;
    async fn find_attempts_by_quiz(
        &self,
        quiz_id: Uuid,
    ) -> Result<Vec<QuizAttempt>, InfrastructureError>;

    // Answers
    async fn save_answer(&self, answer: &AttemptAnswer) -> Result<(), InfrastructureError>;
    async fn find_answers(
        &self,
        attempt_id: Uuid,
    ) -> Result<Vec<AttemptAnswer>, InfrastructureError>;
    async fn find_attempts_by_student(
        &self,
        student_id: Uuid,
        class_id: Uuid,
    ) -> Result<Vec<QuizAttempt>, InfrastructureError>;
}

#[async_trait]
pub trait AssessmentRuleRepository: Send + Sync {
    async fn save(&self, rule: &AssessmentRule) -> Result<(), InfrastructureError>;
    async fn update(&self, rule: &AssessmentRule) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<AssessmentRule>, InfrastructureError>;
    async fn find_by_class_subject(
        &self,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Option<AssessmentRule>, InfrastructureError>;
    async fn find_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<AssessmentRule>, InfrastructureError>;
    async fn save_component(
        &self,
        component: &AssessmentComponent,
    ) -> Result<(), InfrastructureError>;
    async fn clear_components(&self, rule_id: Uuid) -> Result<(), InfrastructureError>;
}

#[async_trait]
pub trait GradebookRepository: Send + Sync {
    async fn save_gradebook(
        &self,
        gradebook: &crate::learning::domain::gradebook::GradeBook,
    ) -> Result<(), InfrastructureError>;
    async fn update_gradebook(
        &self,
        gradebook: &crate::learning::domain::gradebook::GradeBook,
    ) -> Result<(), InfrastructureError>;
    async fn find_gradebook_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<crate::learning::domain::gradebook::GradeBook>, InfrastructureError>;
    async fn find_gradebook_by_student_subject(
        &self,
        student_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Option<crate::learning::domain::gradebook::GradeBook>, InfrastructureError>;
    async fn find_gradebooks_by_class(
        &self,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Vec<crate::learning::domain::gradebook::GradeBook>, InfrastructureError>;
    async fn save_entry(
        &self,
        entry: &crate::learning::domain::gradebook::GradeEntry,
    ) -> Result<(), InfrastructureError>;
    async fn find_entries(
        &self,
        gradebook_id: Uuid,
    ) -> Result<Vec<crate::learning::domain::gradebook::GradeEntry>, InfrastructureError>;
    async fn find_by_class_subject(
        &self,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Vec<GradebookEntry>, InfrastructureError>;
    async fn find_by_student(
        &self,
        student_id: Uuid,
    ) -> Result<Vec<GradebookEntry>, InfrastructureError>;
}

#[async_trait]
pub trait StudentProgressRepository: Send + Sync {
    async fn save(&self, progress: &StudentProgress) -> Result<(), InfrastructureError>;
    async fn find_by_student_class_subject(
        &self,
        student_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Option<StudentProgress>, InfrastructureError>;
    async fn find_by_class(
        &self,
        class_id: Uuid,
    ) -> Result<Vec<StudentProgress>, InfrastructureError>;
}

#[async_trait]
pub trait AchievementRepository: Send + Sync {
    async fn save(&self, achievement: &Achievement) -> Result<(), InfrastructureError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Achievement>, InfrastructureError>;
    async fn find_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<Achievement>, InfrastructureError>;
    async fn delete(&self, id: Uuid) -> Result<(), InfrastructureError>;
    async fn award(&self, sa: &StudentAchievement) -> Result<(), InfrastructureError>;
    async fn find_student_achievements(
        &self,
        student_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Vec<StudentAchievement>, InfrastructureError>;
    async fn find_by_student_and_achievement(
        &self,
        student_id: Uuid,
        achievement_id: Uuid,
    ) -> Result<Option<StudentAchievement>, InfrastructureError>;
}

#[async_trait]
pub trait FeedRepository: Send + Sync {
    async fn create(&self, item: &FeedItem) -> Result<(), InfrastructureError>;
    async fn find_by_class(
        &self,
        class_id: Uuid,
        tenant_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<FeedItem>, InfrastructureError>;
    async fn count_by_class(
        &self,
        class_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<i64, InfrastructureError>;
}
