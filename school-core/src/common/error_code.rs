use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    // Identity & Authentication
    AuthInvalidToken,
    AuthInvalidCredentials,
    AuthTokenExpired,
    AuthPermissionDenied,

    // Academic & People
    StudentDuplicateNisn,
    StudentNotFound,
    StudentAlreadyEnrolled,
    TeacherNotFound,
    GuardianNotFound,
    StaffNotFound,

    // Academic Settings
    AcademicYearClosed,
    AcademicYearNotFound,
    GradeLevelNotFound,
    SubjectNotFound,
    CurriculumNotFound,
    SyllabusNotFound,
    LearningMaterialNotFound,
    LessonNotFound,
    LessonPlanNotFound,
    SessionNotFound,
    AssignmentNotFound,
    SubmissionNotFound,
    QuizNotFound,
    AttemptNotFound,
    AssessmentRulesNotFound,
    ProgressNotFound,
    AchievementNotFound,
    FeedItemNotFound,
    NotificationNotFound,
    ClassFull,

    // Tenant & Core
    TenantAlreadyExists,
    TenantNotFound,
    SchoolNotFound,

    // Infrastructure & System
    ResourceNotFound,
    InternalServerError,
    DatabaseError,
    ValidationFailed,
    SystemMaintenance,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::SystemMaintenance => "SYSTEM_MAINTENANCE",
            ErrorCode::AuthInvalidToken => "AUTH_INVALID_TOKEN",
            ErrorCode::AuthInvalidCredentials => "AUTH_INVALID_CREDENTIALS",
            ErrorCode::AuthTokenExpired => "AUTH_TOKEN_EXPIRED",
            ErrorCode::AuthPermissionDenied => "AUTH_PERMISSION_DENIED",
            ErrorCode::StudentDuplicateNisn => "STUDENT_DUPLICATE_NISN",
            ErrorCode::StudentNotFound => "STUDENT_NOT_FOUND",
            ErrorCode::StudentAlreadyEnrolled => "STUDENT_ALREADY_ENROLLED",
            ErrorCode::TeacherNotFound => "TEACHER_NOT_FOUND",
            ErrorCode::GuardianNotFound => "GUARDIAN_NOT_FOUND",
            ErrorCode::StaffNotFound => "STAFF_NOT_FOUND",
            ErrorCode::AcademicYearClosed => "ACADEMIC_YEAR_CLOSED",
            ErrorCode::AcademicYearNotFound => "ACADEMIC_YEAR_NOT_FOUND",
            ErrorCode::GradeLevelNotFound => "GRADE_LEVEL_NOT_FOUND",
            ErrorCode::SubjectNotFound => "SUBJECT_NOT_FOUND",
            ErrorCode::CurriculumNotFound => "CURRICULUM_NOT_FOUND",
            ErrorCode::SyllabusNotFound => "SYLLABUS_NOT_FOUND",
            ErrorCode::LearningMaterialNotFound => "LEARNING_MATERIAL_NOT_FOUND",
            ErrorCode::LessonNotFound => "LESSON_NOT_FOUND",
            ErrorCode::LessonPlanNotFound => "LESSON_PLAN_NOT_FOUND",
            ErrorCode::SessionNotFound => "SESSION_NOT_FOUND",
            ErrorCode::AssignmentNotFound => "ASSIGNMENT_NOT_FOUND",
            ErrorCode::SubmissionNotFound => "SUBMISSION_NOT_FOUND",
            ErrorCode::QuizNotFound => "QUIZ_NOT_FOUND",
            ErrorCode::AttemptNotFound => "ATTEMPT_NOT_FOUND",
            ErrorCode::AssessmentRulesNotFound => "ASSESSMENT_RULES_NOT_FOUND",
            ErrorCode::ProgressNotFound => "PROGRESS_NOT_FOUND",
            ErrorCode::AchievementNotFound => "ACHIEVEMENT_NOT_FOUND",
            ErrorCode::FeedItemNotFound => "FEED_ITEM_NOT_FOUND",
            ErrorCode::NotificationNotFound => "NOTIFICATION_NOT_FOUND",
            ErrorCode::ClassFull => "CLASS_FULL",
            ErrorCode::TenantAlreadyExists => "TENANT_ALREADY_EXISTS",
            ErrorCode::TenantNotFound => "TENANT_NOT_FOUND",
            ErrorCode::SchoolNotFound => "SCHOOL_NOT_FOUND",
            ErrorCode::ResourceNotFound => "RESOURCE_NOT_FOUND",
            ErrorCode::InternalServerError => "INTERNAL_SERVER_ERROR",
            ErrorCode::DatabaseError => "DATABASE_ERROR",
            ErrorCode::ValidationFailed => "VALIDATION_FAILED",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
