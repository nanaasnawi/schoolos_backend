mod context;

pub use context::ApplicationContext;

use std::sync::Arc;
use std::time::Duration;

use axum::{Router, routing::get};
use sqlx::postgres::PgPoolOptions;
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::middleware::auth_middleware;
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
use school_core::academic::infrastructure::pg_academic_repository::PgAcademicRepository;
use school_core::audit::application::event_subscriber::AuditEventSubscriber;
use school_core::audit::infrastructure::pg_audit_repository::PgAuditRepository;
use school_core::common::application::outbox::OutboxDispatcher;
use school_core::common::domain::clock::{Clock, SystemClock};
use school_core::common::event_bus::{InMemoryEventBus, SharedEventBus};
use school_core::common::infrastructure::pg_outbox_repository::PgOutboxRepository;
use school_core::common::infrastructure::pg_uow::PgUnitOfWorkFactory;
use school_core::identity::application::auth::authenticate_user::AuthenticateUserUseCase;
use school_core::identity::application::auth::authenticate_qr_token::AuthenticateQrTokenUseCase;
use school_core::identity::application::auth::generate_qr_token::GenerateQrTokenUseCase;
use school_core::identity::application::auth::register_user::RegisterUserUseCase;

use school_core::identity::application::tenant::provision_tenant::ProvisionTenantUseCase;
use school_core::identity::infrastructure::{
    pg_tenant_repository::PgTenantRepository, pg_user_repository::PgUserRepository,
};
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
use school_core::learning::infrastructure::pg_achievement_repository::PgAchievementRepository;
use school_core::learning::infrastructure::pg_assessment_repository::PgAssessmentRepository;
use school_core::learning::infrastructure::pg_assignment_repository::PgAssignmentRepository;
use school_core::learning::infrastructure::pg_curriculum_repository::PgCurriculumRepository;
use school_core::learning::infrastructure::pg_feed_repository::PgFeedRepository;
use school_core::learning::infrastructure::pg_learning_material_repository::PgLearningMaterialRepository;
use school_core::learning::infrastructure::pg_lesson_repository::PgLessonRepository;
use school_core::learning::infrastructure::pg_progress_repository::PgStudentProgressRepository;
use school_core::learning::infrastructure::pg_quiz_repository::PgQuizRepository;
use school_core::learning::infrastructure::pg_session_repository::PgSessionRepository;
use school_core::learning::infrastructure::pg_syllabus_repository::PgSyllabusRepository;
use school_core::notification::application::{
    get_preferences::GetPreferencesUseCase, list_notifications::ListNotificationsUseCase,
    mark_all_read::MarkAllNotificationsReadUseCase, mark_read::MarkNotificationReadUseCase,
    upsert_preference::UpsertPreferenceUseCase,
};
use school_core::notification::infrastructure::notification_subscriber::NotificationEventSubscriber;
use school_core::notification::infrastructure::pg_notification_repository::PgNotificationRepository;
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
use school_core::people::infrastructure::pg_people_repository::PgPeopleRepository;
use school_core::permission::infrastructure::pg_permission_repository::PgRoleRepository;

use crate::ApiDoc;
use crate::idempotency;
use crate::infrastructure::observability::metrics::{
    metrics_handler, metrics_middleware, setup_metrics_recorder,
};
use crate::infrastructure::observability::tracing::tracing_middleware;
use crate::presentation::system::controller::system_routes;
use crate::presentation::{
    academic::academic_years::controller::academic_year_routes,
    academic::classes::controller::class_routes,
    academic::enrollments::controller::enrollment_routes,
    academic::grade_levels::controller::grade_level_routes,
    academic::subjects::controller::subject_routes, academic::terms::controller::term_routes,
    analytics::controller::analytics_routes,
    auth::controller::auth_routes, dapodik::controller::dapodik_routes, health::controller::health_routes,
    learning::achievement::controller::achievement_routes,
    learning::assessment::controller::assessment_routes,
    learning::assignments::controller::assignment_routes,
    learning::curricula::controller::curriculum_routes, learning::feed::controller::feed_routes,
    learning::lessons::controller::lesson_routes, learning::materials::controller::material_routes,
    learning::progress::controller::progress_routes, learning::quizzes::controller::quiz_routes,
    learning::sessions::controller::session_routes,
    learning::syllabuses::controller::syllabus_routes,
    notifications::controller::notification_routes, people::guardian::controller::guardian_routes,
    people::staff::controller::staff_routes, people::students::controller::student_routes,
    people::teacher::controller::teacher_routes, school::controller::school_routes,
    school::controller::get_school_public_info,
    tenant::controller::tenant_routes,
};

/// Composition root for the API server.
///
/// Wires infrastructure, repositories, use cases, background workers, and the HTTP router.
pub struct Bootstrap {
    database_url: String,
    jwt_secret: String,
    outbox_poll_interval: Duration,
    event_bus_capacity: usize,
}

impl Bootstrap {
    pub fn new() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://school_admin:secretpassword@localhost:5432/school_os".to_string()
            }),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "super_secret_jwt_key_123".to_string()),
            outbox_poll_interval: Duration::from_millis(500),
            event_bus_capacity: 100,
        }
    }

    pub fn with_database_url(mut self, url: impl Into<String>) -> Self {
        self.database_url = url.into();
        self
    }

    pub fn with_jwt_secret(mut self, secret: impl Into<String>) -> Self {
        self.jwt_secret = secret.into();
        self
    }

    pub fn with_outbox_poll_interval(mut self, interval: Duration) -> Self {
        self.outbox_poll_interval = interval;
        self
    }

    pub async fn build(self) -> Result<Router, Box<dyn std::error::Error>> {
        let prometheus_handle = setup_metrics_recorder();

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&self.database_url)
            .await?;

        // Infrastructure
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let in_memory_event_bus = Arc::new(InMemoryEventBus::new(self.event_bus_capacity));
        let event_bus: SharedEventBus = in_memory_event_bus.clone();
        let outbox_repo = Arc::new(PgOutboxRepository::new(pool.clone()));
        let uow_factory = Arc::new(PgUnitOfWorkFactory::new(pool.clone()));

        let outbox_dispatcher = Arc::new(OutboxDispatcher::new(
            outbox_repo.clone(),
            event_bus.clone(),
            clock.clone(),
            self.outbox_poll_interval,
        ));
        tokio::spawn({
            let dispatcher = outbox_dispatcher.clone();
            async move {
                dispatcher.start().await;
            }
        });

        // Repositories
        let user_repo = Arc::new(PgUserRepository::new(pool.clone()));
        let _tenant_repo = Arc::new(PgTenantRepository::new(pool.clone()));
        let student_repo = Arc::new(PgPeopleRepository::new(pool.clone()));
        let teacher_repo = Arc::new(PgPeopleRepository::new(pool.clone()));
        let guardian_repo = Arc::new(PgPeopleRepository::new(pool.clone()));
        let staff_repo = Arc::new(PgPeopleRepository::new(pool.clone()));
        let audit_repo = Arc::new(PgAuditRepository::new(pool.clone()));
        let academic_repo = Arc::new(PgAcademicRepository::new(pool.clone()));
        let curriculum_repo = Arc::new(PgCurriculumRepository::new(pool.clone()));
        let syllabus_repo = Arc::new(PgSyllabusRepository::new(pool.clone()));
        let learning_material_repo = Arc::new(PgLearningMaterialRepository::new(pool.clone()));
        let lesson_repo = Arc::new(PgLessonRepository::new(pool.clone()));
        let session_repo = Arc::new(PgSessionRepository::new(pool.clone()));
        let assignment_repo = Arc::new(PgAssignmentRepository::new(pool.clone()));
        let quiz_repo = Arc::new(PgQuizRepository::new(pool.clone()));
        let assessment_repo = Arc::new(PgAssessmentRepository::new(pool.clone()));
        let progress_repo = Arc::new(PgStudentProgressRepository::new(pool.clone()));
        let achievement_repo = Arc::new(PgAchievementRepository::new(pool.clone()));
        let feed_repo = Arc::new(PgFeedRepository::new(pool.clone()));
        let pg_notification = Arc::new(PgNotificationRepository::new(pool.clone()));
        let notification_repo: Arc<dyn NotificationRepository> = pg_notification.clone();
        let notification_pref_repo: Arc<dyn NotificationPreferenceRepository> = pg_notification;
        let role_repo = Arc::new(PgRoleRepository::new(pool.clone()));

        AuditEventSubscriber::start(in_memory_event_bus.clone(), audit_repo, clock.clone());
        NotificationEventSubscriber::start(in_memory_event_bus, notification_repo.clone());

        // Clone everything before any moves to avoid ownership issues
        let c1 = clock.clone();
        let c2 = clock.clone();
        let c3 = clock.clone();
        let c4 = clock.clone();
        let c5 = clock.clone();
        let c6 = clock.clone();
        let c7 = clock.clone();
        let c8 = clock.clone();
        let c9 = clock.clone();
        let c10 = clock.clone();
        let c11 = clock.clone();
        let c12 = clock.clone();
        let c13 = clock.clone();

        let oc1 = outbox_repo.clone();
        let oc2 = outbox_repo.clone();
        let oc3 = outbox_repo.clone();
        let oc4 = outbox_repo.clone();
        let oc5 = outbox_repo.clone();
        let oc6 = outbox_repo.clone();
        let oc7 = outbox_repo.clone();
        let o_final = outbox_repo;

        let uc1 = uow_factory.clone();
        let uc2 = uow_factory.clone();
        let uc3 = uow_factory.clone();
        let uc4 = uow_factory.clone();
        let uc5 = uow_factory.clone();
        let uc6 = uow_factory.clone();
        let uc7 = uow_factory.clone();
        let u_final = uow_factory;

        let event_bus_for_sessions = event_bus.clone();
        let event_bus_for_assignments = event_bus.clone();
        let event_bus_for_grading = event_bus.clone();
        let event_bus_for_quiz_publish = event_bus.clone();
        let event_bus_for_attempt_start = event_bus.clone();
        let event_bus_for_attempt_submit = event_bus.clone();
        let event_bus_for_grade = event_bus.clone();
        let event_bus_for_assessment = event_bus.clone();
        let event_bus_for_progress = event_bus.clone();
        let event_bus_for_achievement = event_bus.clone();

        // Application context (service registry)
        let context = ApplicationContext {
            pool: pool.clone(),
            event_bus,
            clock: clock.clone(),
            authenticate_user: Arc::new(AuthenticateUserUseCase::new(
                user_repo.clone(),
                self.jwt_secret.clone(),
                clock.clone(),
            )),
            authenticate_qr_token: Arc::new(AuthenticateQrTokenUseCase::new(
                pool.clone(),
                self.jwt_secret.clone(),
                clock.clone(),
            )),
            generate_qr_token: Arc::new(GenerateQrTokenUseCase::new(pool.clone(), clock.clone())),
            register_user: Arc::new(RegisterUserUseCase::new(user_repo.clone(), clock.clone())),

            provision_tenant: Arc::new(ProvisionTenantUseCase::new(pool.clone(), clock.clone())),
            role_repo: role_repo.clone(),
            create_student: Arc::new(CreateStudentUseCase::new(
                student_repo.clone(),
                oc1,
                uc1,
                clock.clone(),
            )),
            update_student: Arc::new(UpdateStudentUseCase::new(
                student_repo.clone(),
                o_final,
                u_final,
                clock.clone(),
            )),
            get_student: Arc::new(GetStudentProfileUseCase::new(student_repo.clone())),
            list_students: Arc::new(ListStudentsUseCase::new(student_repo.clone())),
            create_teacher: Arc::new(CreateTeacherUseCase::new(
                teacher_repo.clone(),
                oc2,
                uc2,
                clock.clone(),
            )),
            update_teacher: Arc::new(UpdateTeacherUseCase::new(
                teacher_repo.clone(),
                oc3,
                uc3,
                clock.clone(),
            )),
            get_teacher: Arc::new(GetTeacherUseCase::new(teacher_repo.clone())),
            list_teachers: Arc::new(ListTeachersUseCase::new(teacher_repo.clone())),
            create_guardian: Arc::new(CreateGuardianUseCase::new(
                guardian_repo.clone(),
                oc4,
                uc4,
                clock.clone(),
            )),
            update_guardian: Arc::new(UpdateGuardianUseCase::new(
                guardian_repo.clone(),
                oc5,
                uc5,
                clock.clone(),
            )),
            get_guardian: Arc::new(GetGuardianUseCase::new(guardian_repo.clone())),
            list_guardians: Arc::new(ListGuardiansUseCase::new(guardian_repo.clone())),
            create_staff: Arc::new(CreateStaffUseCase::new(
                staff_repo.clone(),
                oc6,
                uc6,
                clock.clone(),
            )),
            update_staff: Arc::new(UpdateStaffUseCase::new(
                staff_repo.clone(),
                oc7,
                uc7,
                clock.clone(),
            )),
            get_staff: Arc::new(GetStaffUseCase::new(staff_repo.clone())),
            list_staff: Arc::new(ListStaffUseCase::new(staff_repo.clone())),
            create_academic_year: Arc::new(CreateAcademicYearUseCase::new(
                academic_repo.clone(),
                clock.clone(),
            )),
            list_academic_years: Arc::new(ListAcademicYearsUseCase::new(academic_repo.clone())),
            create_class: Arc::new(CreateClassUseCase::new(
                academic_repo.clone(),
                academic_repo.clone(),
                clock.clone(),
            )),
            list_classes: Arc::new(ListClassesUseCase::new(academic_repo.clone())),
            enroll_student: Arc::new(EnrollStudentUseCase::new(
                academic_repo.clone(),
                academic_repo.clone(),
                student_repo,
                clock.clone(),
            )),
            list_enrollments: Arc::new(ListEnrollmentsUseCase::new(academic_repo.clone())),
            create_subject: Arc::new(CreateSubjectUseCase::new(academic_repo.clone(), c1)),
            list_subjects: Arc::new(ListSubjectsUseCase::new(academic_repo.clone())),
            get_subject: Arc::new(GetSubjectUseCase::new(academic_repo.clone())),
            create_grade_level: Arc::new(CreateGradeLevelUseCase::new(academic_repo.clone(), c2)),
            list_grade_levels: Arc::new(ListGradeLevelsUseCase::new(academic_repo.clone())),
            get_grade_level: Arc::new(GetGradeLevelUseCase::new(academic_repo.clone())),
            create_term: Arc::new(CreateTermUseCase::new(academic_repo.clone(), c3)),
            list_terms: Arc::new(ListTermsUseCase::new(academic_repo.clone())),
            get_term: Arc::new(GetTermUseCase::new(academic_repo.clone())),

            create_curriculum: Arc::new(CreateCurriculumUseCase::new(curriculum_repo.clone(), c4)),
            list_curricula: Arc::new(ListCurriculaUseCase::new(curriculum_repo.clone())),
            get_curriculum: Arc::new(GetCurriculumUseCase::new(curriculum_repo)),

            create_syllabus: Arc::new(CreateSyllabusUseCase::new(syllabus_repo.clone(), c5)),
            list_syllabuses: Arc::new(ListSyllabusesUseCase::new(syllabus_repo.clone())),
            get_syllabus: Arc::new(GetSyllabusUseCase::new(syllabus_repo.clone())),
            add_competency: Arc::new(AddCompetencyUseCase::new(syllabus_repo.clone(), c6)),
            list_competencies: Arc::new(ListCompetenciesUseCase::new(syllabus_repo)),

            create_learning_material: Arc::new(CreateLearningMaterialUseCase::new(
                learning_material_repo.clone(),
                c7,
            )),
            list_learning_materials: Arc::new(ListLearningMaterialsUseCase::new(
                learning_material_repo.clone(),
            )),
            get_learning_material: Arc::new(GetLearningMaterialUseCase::new(
                learning_material_repo.clone(),
            )),
            update_learning_material: Arc::new(UpdateLearningMaterialUseCase::new(
                learning_material_repo.clone(),
            )),
            delete_learning_material: Arc::new(DeleteLearningMaterialUseCase::new(
                learning_material_repo.clone(),
            )),

            create_lesson: Arc::new(CreateLessonUseCase::new(lesson_repo.clone(), c8)),
            list_lessons: Arc::new(ListLessonsUseCase::new(lesson_repo.clone())),
            get_lesson: Arc::new(GetLessonUseCase::new(lesson_repo.clone())),
            update_lesson: Arc::new(UpdateLessonUseCase::new(lesson_repo.clone(), clock.clone())),
            publish_lesson: Arc::new(PublishLessonUseCase::new(
                lesson_repo.clone(),
                learning_material_repo.clone(),
                clock.clone(),
            )),
            archive_lesson: Arc::new(ArchiveLessonUseCase::new(
                lesson_repo.clone(),
                clock.clone(),
            )),
            delete_lesson: Arc::new(DeleteLessonUseCase::new(lesson_repo.clone())),
            create_lesson_plan: Arc::new(CreateLessonPlanUseCase::new(lesson_repo.clone())),
            get_lesson_plan: Arc::new(GetLessonPlanUseCase::new(lesson_repo.clone())),

            start_session: Arc::new(StartSessionUseCase::new(
                session_repo.clone(),
                c9,
                event_bus_for_sessions.clone(),
            )),
            end_session: Arc::new(EndSessionUseCase::new(
                session_repo.clone(),
                clock.clone(),
                event_bus_for_sessions.clone(),
            )),
            get_session: Arc::new(GetSessionUseCase::new(session_repo.clone())),
            list_sessions: Arc::new(ListSessionsUseCase::new(session_repo.clone())),
            record_attendance: Arc::new(RecordAttendanceUseCase::new(session_repo.clone())),
            get_attendance: Arc::new(GetAttendanceUseCase::new(session_repo.clone())),

            create_assignment: Arc::new(CreateAssignmentUseCase::new(assignment_repo.clone(), c10)),
            list_assignments: Arc::new(ListAssignmentsUseCase::new(assignment_repo.clone())),
            get_assignment: Arc::new(GetAssignmentUseCase::new(assignment_repo.clone())),
            update_assignment: Arc::new(UpdateAssignmentUseCase::new(
                assignment_repo.clone(),
                clock.clone(),
            )),
            publish_assignment: Arc::new(PublishAssignmentUseCase::new(
                assignment_repo.clone(),
                lesson_repo.clone(),
                clock.clone(),
            )),
            close_assignment: Arc::new(CloseAssignmentUseCase::new(
                assignment_repo.clone(),
                clock.clone(),
            )),
            archive_assignment: Arc::new(ArchiveAssignmentUseCase::new(
                assignment_repo.clone(),
                clock.clone(),
            )),
            delete_assignment: Arc::new(DeleteAssignmentUseCase::new(assignment_repo.clone())),
            submit_assignment: Arc::new(SubmitAssignmentUseCase::new(
                assignment_repo.clone(),
                clock.clone(),
                event_bus_for_assignments.clone(),
            )),
            grade_submission: Arc::new(GradeSubmissionUseCase::new(
                assignment_repo.clone(),
                c11,
                event_bus_for_grading.clone(),
            )),
            get_submissions: Arc::new(GetSubmissionsUseCase::new(assignment_repo)),

            create_quiz: Arc::new(CreateQuizUseCase::new(quiz_repo.clone(), c12)),
            list_quizzes: Arc::new(ListQuizzesUseCase::new(quiz_repo.clone())),
            get_quiz: Arc::new(GetQuizUseCase::new(quiz_repo.clone())),
            publish_quiz: Arc::new(PublishQuizUseCase::new(
                quiz_repo.clone(),
                lesson_repo.clone(),
                clock.clone(),
                event_bus_for_quiz_publish.clone(),
            )),
            start_attempt: Arc::new(StartAttemptUseCase::new(
                quiz_repo.clone(),
                clock.clone(),
                event_bus_for_attempt_start.clone(),
            )),
            submit_attempt: Arc::new(SubmitAttemptUseCase::new(
                quiz_repo.clone(),
                clock.clone(),
                event_bus_for_attempt_submit.clone(),
            )),
            grade_attempt: Arc::new(GradeAttemptUseCase::new(
                quiz_repo,
                clock.clone(),
                event_bus_for_grade.clone(),
            )),

            configure_assessment_rules: Arc::new(ConfigureRulesUseCase::new(
                assessment_repo.clone(),
                c13,
                event_bus_for_assessment.clone(),
            )),
            get_assessment_rules: Arc::new(GetRulesUseCase::new(assessment_repo.clone())),
            calculate_grade: Arc::new(CalculateGradeUseCase::new(
                assessment_repo.clone(),
                assessment_repo.clone(),
                clock.clone(),
                event_bus_for_assessment.clone(),
            )),
            get_gradebook: Arc::new(GetGradebookUseCase::new(assessment_repo)),

            calculate_progress: Arc::new(CalculateProgressUseCase::new(
                progress_repo.clone(),
                lesson_repo.clone(),
                session_repo.clone(),
                event_bus_for_progress.clone(),
            )),
            get_progress: Arc::new(GetProgressUseCase::new(progress_repo)),

            create_achievement: Arc::new(CreateAchievementUseCase::new(
                achievement_repo.clone(),
                clock.clone(),
                event_bus_for_achievement.clone(),
            )),
            list_achievements: Arc::new(ListAchievementsUseCase::new(achievement_repo.clone())),
            get_achievement: Arc::new(GetAchievementUseCase::new(achievement_repo.clone())),
            award_achievement: Arc::new(AwardAchievementUseCase::new(
                achievement_repo.clone(),
                event_bus_for_achievement.clone(),
            )),
            get_student_achievements: Arc::new(GetStudentAchievementsUseCase::new(
                achievement_repo,
            )),

            create_feed_item: Arc::new(CreateFeedItemUseCase::new(feed_repo.clone())),
            list_feed: Arc::new(ListFeedUseCase::new(feed_repo)),

            notification_repo: notification_repo.clone(),
            notification_pref_repo: notification_pref_repo.clone(),
            list_notifications: Arc::new(ListNotificationsUseCase::new(notification_repo.clone())),
            mark_notification_read: Arc::new(MarkNotificationReadUseCase::new(
                notification_repo.clone(),
            )),
            mark_all_notifications_read: Arc::new(MarkAllNotificationsReadUseCase::new(
                notification_repo.clone(),
            )),
            upsert_notification_preference: Arc::new(UpsertPreferenceUseCase::new(
                notification_pref_repo.clone(),
            )),
            get_notification_preferences: Arc::new(GetPreferencesUseCase::new(
                notification_pref_repo,
            )),
        };

        let app = Router::new()
            .nest("/api/v1/auth", auth_routes(context.clone()))
            .nest(
                "/api/v1/analytics",
                analytics_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest("/api/v1/tenants", tenant_routes())
            // Public school info (no auth required) — used by mobile login screen
            .route(
                "/api/v1/schools/info",
                axum::routing::get(get_school_public_info).with_state(context.clone()),
            )
            .nest(
                "/api/v1/schools",
                school_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/students",
                student_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/teachers",
                teacher_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/guardians",
                guardian_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/staff",
                staff_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/academic/years",
                academic_year_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/academic/classes",
                class_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/academic/enrollments",
                enrollment_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/academic/subjects",
                subject_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/academic/grade-levels",
                grade_level_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/academic/terms",
                term_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/curricula",
                curriculum_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/syllabuses",
                syllabus_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/materials",
                material_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/lessons",
                lesson_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/sessions",
                session_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/assignments",
                assignment_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/quizzes",
                quiz_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/assessment",
                assessment_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/progress",
                progress_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/achievements",
                achievement_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/learning/feed",
                feed_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/notifications",
                notification_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/dapodik",
                dapodik_routes()
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        idempotency::idempotency_middleware,
                    ))
                    .layer(axum::middleware::from_fn_with_state(
                        context.clone(),
                        auth_middleware,
                    )),
            )
            .nest(
                "/api/v1/system",
                system_routes(context.clone())
            )
            .nest("/health", health_routes())
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            .route(
                "/metrics",
                get(metrics_handler).with_state(prometheus_handle),
            )
            .layer(CorsLayer::permissive())
            .layer(axum::middleware::from_fn(metrics_middleware))
            .layer(axum::middleware::from_fn(tracing_middleware))
            .layer(TraceLayer::new_for_http())
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(PropagateRequestIdLayer::x_request_id())
            .with_state(context);

        Ok(app)
    }
}

impl Default for Bootstrap {
    fn default() -> Self {
        Self::new()
    }
}
