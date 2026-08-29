use std::sync::Arc;

use sqlx::PgPool;

use school_core::academic::application::academic_year::{
    create_academic_year::CreateAcademicYearUseCase, list_academic_years::ListAcademicYearsUseCase,
};
use school_core::academic::application::class::{
    create_class::CreateClassUseCase, list_classes::ListClassesUseCase,
};
use school_core::academic::application::enrollment::{
    enroll_student::EnrollStudentUseCase, list_enrollments::ListEnrollmentsUseCase,
};
use school_core::academic::application::grade_level::{
    create_grade_level::CreateGradeLevelUseCase, get_grade_level::GetGradeLevelUseCase,
    list_grade_levels::ListGradeLevelsUseCase,
};
use school_core::academic::application::subject::{
    create_subject::CreateSubjectUseCase, get_subject::GetSubjectUseCase,
    list_subjects::ListSubjectsUseCase,
};
use school_core::academic::application::term::{
    create_term::CreateTermUseCase, get_term::GetTermUseCase, list_terms::ListTermsUseCase,
};
use school_core::common::domain::clock::Clock;
use school_core::common::event_bus::SharedEventBus;
use school_core::identity::application::auth::authenticate_user::AuthenticateUserUseCase;
use school_core::identity::application::auth::authenticate_qr_token::AuthenticateQrTokenUseCase;
use school_core::identity::application::auth::generate_qr_token::GenerateQrTokenUseCase;
use school_core::identity::application::auth::register_user::RegisterUserUseCase;

use school_core::identity::application::tenant::provision_tenant::ProvisionTenantUseCase;
use school_core::learning::application::achievement::{
    award_achievement::AwardAchievementUseCase, create_achievement::CreateAchievementUseCase,
    get_achievement::GetAchievementUseCase,
    get_student_achievements::GetStudentAchievementsUseCase,
    list_achievements::ListAchievementsUseCase,
};
use school_core::learning::application::assessment::{
    calculate_grade::CalculateGradeUseCase, configure_rules::ConfigureRulesUseCase,
    get_gradebook::GetGradebookUseCase, get_rules::GetRulesUseCase,
};
use school_core::learning::application::assignment::{
    archive_assignment::ArchiveAssignmentUseCase, close_assignment::CloseAssignmentUseCase,
    create_assignment::CreateAssignmentUseCase, delete_assignment::DeleteAssignmentUseCase,
    get_assignment::GetAssignmentUseCase, get_submissions::GetSubmissionsUseCase,
    grade_submission::GradeSubmissionUseCase, list_assignments::ListAssignmentsUseCase,
    publish_assignment::PublishAssignmentUseCase, submit_assignment::SubmitAssignmentUseCase,
    update_assignment::UpdateAssignmentUseCase,
};
use school_core::learning::application::curriculum::{
    create_curriculum::CreateCurriculumUseCase, get_curriculum::GetCurriculumUseCase,
    list_curricula::ListCurriculaUseCase,
};
use school_core::learning::application::feed::{
    create_feed_item::CreateFeedItemUseCase, list_feed::ListFeedUseCase,
};
use school_core::learning::application::learning_material::{
    create_learning_material::CreateLearningMaterialUseCase,
    delete_learning_material::DeleteLearningMaterialUseCase,
    get_learning_material::GetLearningMaterialUseCase,
    list_learning_materials::ListLearningMaterialsUseCase,
    update_learning_material::UpdateLearningMaterialUseCase,
};
use school_core::learning::application::lesson::{
    archive_lesson::ArchiveLessonUseCase, create_lesson::CreateLessonUseCase,
    create_lesson_plan::CreateLessonPlanUseCase, delete_lesson::DeleteLessonUseCase,
    get_lesson::GetLessonUseCase, get_lesson_plan::GetLessonPlanUseCase,
    list_lessons::ListLessonsUseCase, publish_lesson::PublishLessonUseCase,
    update_lesson::UpdateLessonUseCase,
};
use school_core::learning::application::progress::{
    calculate_progress::CalculateProgressUseCase, get_progress::GetProgressUseCase,
};
use school_core::learning::application::quiz::{
    create_quiz::CreateQuizUseCase, get_quiz::GetQuizUseCase, grade_attempt::GradeAttemptUseCase,
    list_quizzes::ListQuizzesUseCase, publish_quiz::PublishQuizUseCase,
    start_attempt::StartAttemptUseCase, submit_attempt::SubmitAttemptUseCase,
};
use school_core::learning::application::session::{
    end_session::EndSessionUseCase, get_attendance::GetAttendanceUseCase,
    get_session::GetSessionUseCase, list_sessions::ListSessionsUseCase,
    record_attendance::RecordAttendanceUseCase, start_session::StartSessionUseCase,
};
use school_core::learning::application::syllabus::{
    add_competency::AddCompetencyUseCase, create_syllabus::CreateSyllabusUseCase,
    get_syllabus::GetSyllabusUseCase, list_competencies::ListCompetenciesUseCase,
    list_syllabuses::ListSyllabusesUseCase,
};
use school_core::notification::application::{
    get_preferences::GetPreferencesUseCase, list_notifications::ListNotificationsUseCase,
    mark_all_read::MarkAllNotificationsReadUseCase, mark_read::MarkNotificationReadUseCase,
    upsert_preference::UpsertPreferenceUseCase,
};
use school_core::notification::infrastructure::repository_traits::{
    NotificationPreferenceRepository, NotificationRepository,
};
use school_core::people::application::create_student::handler::CreateStudentUseCase;
use school_core::people::application::get_student_profile::handler::GetStudentProfileUseCase;
use school_core::people::application::guardian::create::CreateGuardianUseCase;
use school_core::people::application::guardian::get::GetGuardianUseCase;
use school_core::people::application::guardian::list::ListGuardiansUseCase;
use school_core::people::application::guardian::update::UpdateGuardianUseCase;
use school_core::people::application::list_students::handler::ListStudentsUseCase;
use school_core::people::application::staff::create::CreateStaffUseCase;
use school_core::people::application::staff::get::GetStaffUseCase;
use school_core::people::application::staff::list::ListStaffUseCase;
use school_core::people::application::staff::update::UpdateStaffUseCase;
use school_core::people::application::teacher::create::CreateTeacherUseCase;
use school_core::people::application::teacher::get::GetTeacherUseCase;
use school_core::people::application::teacher::list::ListTeachersUseCase;
use school_core::people::application::teacher::update::UpdateTeacherUseCase;
use school_core::people::application::update_student::handler::UpdateStudentUseCase;
use school_core::permission::infrastructure::repository_traits::RoleRepository;

/// Central service registry for the API server composition root.
///
/// Holds shared infrastructure and all application use cases, replacing
/// per-domain `*AppState` structs scattered across presentation controllers.
#[derive(Clone)]
pub struct ApplicationContext {
    pub pool: PgPool,
    pub event_bus: SharedEventBus,
    pub clock: Arc<dyn Clock>,

    pub authenticate_user: Arc<AuthenticateUserUseCase>,
    pub authenticate_qr_token: Arc<AuthenticateQrTokenUseCase>,
    pub generate_qr_token: Arc<GenerateQrTokenUseCase>,
    pub register_user: Arc<RegisterUserUseCase>,

    pub provision_tenant: Arc<ProvisionTenantUseCase>,

    pub role_repo: Arc<dyn RoleRepository>,

    pub create_student: Arc<CreateStudentUseCase>,
    pub update_student: Arc<UpdateStudentUseCase>,
    pub get_student: Arc<GetStudentProfileUseCase>,
    pub list_students: Arc<ListStudentsUseCase>,

    pub create_teacher: Arc<CreateTeacherUseCase>,
    pub update_teacher: Arc<UpdateTeacherUseCase>,
    pub get_teacher: Arc<GetTeacherUseCase>,
    pub list_teachers: Arc<ListTeachersUseCase>,

    pub create_guardian: Arc<CreateGuardianUseCase>,
    pub update_guardian: Arc<UpdateGuardianUseCase>,
    pub get_guardian: Arc<GetGuardianUseCase>,
    pub list_guardians: Arc<ListGuardiansUseCase>,

    pub create_staff: Arc<CreateStaffUseCase>,
    pub update_staff: Arc<UpdateStaffUseCase>,
    pub get_staff: Arc<GetStaffUseCase>,
    pub list_staff: Arc<ListStaffUseCase>,

    pub create_academic_year: Arc<CreateAcademicYearUseCase>,
    pub list_academic_years: Arc<ListAcademicYearsUseCase>,

    pub create_class: Arc<CreateClassUseCase>,
    pub list_classes: Arc<ListClassesUseCase>,

    pub enroll_student: Arc<EnrollStudentUseCase>,
    pub list_enrollments: Arc<ListEnrollmentsUseCase>,

    pub create_subject: Arc<CreateSubjectUseCase>,
    pub list_subjects: Arc<ListSubjectsUseCase>,
    pub get_subject: Arc<GetSubjectUseCase>,

    pub create_grade_level: Arc<CreateGradeLevelUseCase>,
    pub list_grade_levels: Arc<ListGradeLevelsUseCase>,
    pub get_grade_level: Arc<GetGradeLevelUseCase>,

    pub create_term: Arc<CreateTermUseCase>,
    pub list_terms: Arc<ListTermsUseCase>,
    pub get_term: Arc<GetTermUseCase>,

    pub create_curriculum: Arc<CreateCurriculumUseCase>,
    pub list_curricula: Arc<ListCurriculaUseCase>,
    pub get_curriculum: Arc<GetCurriculumUseCase>,

    pub create_syllabus: Arc<CreateSyllabusUseCase>,
    pub list_syllabuses: Arc<ListSyllabusesUseCase>,
    pub get_syllabus: Arc<GetSyllabusUseCase>,
    pub add_competency: Arc<AddCompetencyUseCase>,
    pub list_competencies: Arc<ListCompetenciesUseCase>,

    pub create_learning_material: Arc<CreateLearningMaterialUseCase>,
    pub list_learning_materials: Arc<ListLearningMaterialsUseCase>,
    pub get_learning_material: Arc<GetLearningMaterialUseCase>,
    pub update_learning_material: Arc<UpdateLearningMaterialUseCase>,
    pub delete_learning_material: Arc<DeleteLearningMaterialUseCase>,

    pub create_lesson: Arc<CreateLessonUseCase>,
    pub list_lessons: Arc<ListLessonsUseCase>,
    pub get_lesson: Arc<GetLessonUseCase>,
    pub update_lesson: Arc<UpdateLessonUseCase>,
    pub publish_lesson: Arc<PublishLessonUseCase>,
    pub archive_lesson: Arc<ArchiveLessonUseCase>,
    pub delete_lesson: Arc<DeleteLessonUseCase>,
    pub create_lesson_plan: Arc<CreateLessonPlanUseCase>,
    pub get_lesson_plan: Arc<GetLessonPlanUseCase>,

    pub start_session: Arc<StartSessionUseCase>,
    pub end_session: Arc<EndSessionUseCase>,
    pub get_session: Arc<GetSessionUseCase>,
    pub list_sessions: Arc<ListSessionsUseCase>,
    pub record_attendance: Arc<RecordAttendanceUseCase>,
    pub get_attendance: Arc<GetAttendanceUseCase>,

    pub create_assignment: Arc<CreateAssignmentUseCase>,
    pub list_assignments: Arc<ListAssignmentsUseCase>,
    pub get_assignment: Arc<GetAssignmentUseCase>,
    pub update_assignment: Arc<UpdateAssignmentUseCase>,
    pub publish_assignment: Arc<PublishAssignmentUseCase>,
    pub close_assignment: Arc<CloseAssignmentUseCase>,
    pub archive_assignment: Arc<ArchiveAssignmentUseCase>,
    pub delete_assignment: Arc<DeleteAssignmentUseCase>,
    pub submit_assignment: Arc<SubmitAssignmentUseCase>,
    pub grade_submission: Arc<GradeSubmissionUseCase>,
    pub get_submissions: Arc<GetSubmissionsUseCase>,

    pub create_quiz: Arc<CreateQuizUseCase>,
    pub list_quizzes: Arc<ListQuizzesUseCase>,
    pub get_quiz: Arc<GetQuizUseCase>,
    pub publish_quiz: Arc<PublishQuizUseCase>,
    pub start_attempt: Arc<StartAttemptUseCase>,
    pub submit_attempt: Arc<SubmitAttemptUseCase>,
    pub grade_attempt: Arc<GradeAttemptUseCase>,

    pub configure_assessment_rules: Arc<ConfigureRulesUseCase>,
    pub get_assessment_rules: Arc<GetRulesUseCase>,
    pub calculate_grade: Arc<CalculateGradeUseCase>,
    pub get_gradebook: Arc<GetGradebookUseCase>,

    pub calculate_progress: Arc<CalculateProgressUseCase>,
    pub get_progress: Arc<GetProgressUseCase>,

    pub create_achievement: Arc<CreateAchievementUseCase>,
    pub list_achievements: Arc<ListAchievementsUseCase>,
    pub get_achievement: Arc<GetAchievementUseCase>,
    pub award_achievement: Arc<AwardAchievementUseCase>,
    pub get_student_achievements: Arc<GetStudentAchievementsUseCase>,

    pub create_feed_item: Arc<CreateFeedItemUseCase>,
    pub list_feed: Arc<ListFeedUseCase>,

    pub notification_repo: Arc<dyn NotificationRepository>,
    pub notification_pref_repo: Arc<dyn NotificationPreferenceRepository>,
    pub list_notifications: Arc<ListNotificationsUseCase>,
    pub mark_notification_read: Arc<MarkNotificationReadUseCase>,
    pub mark_all_notifications_read: Arc<MarkAllNotificationsReadUseCase>,
    pub upsert_notification_preference: Arc<UpsertPreferenceUseCase>,
    pub get_notification_preferences: Arc<GetPreferencesUseCase>,
}
