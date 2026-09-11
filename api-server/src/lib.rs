pub mod bootstrap;
pub mod error;
pub mod extractors;
pub mod idempotency;
pub mod idempotency_cleanup;
pub mod infrastructure;
pub mod middleware;
pub mod presentation;
pub mod response;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::presentation::auth::controller::login,
        crate::presentation::tenant::controller::provision,
        crate::presentation::school::controller::get_school,
        crate::presentation::school::controller::get_current_school_profile,
        crate::presentation::school::controller::update_current_school_profile,
        crate::presentation::people::students::controller::create,
        crate::presentation::people::students::controller::list,
        crate::presentation::people::students::controller::get_by_id,
        crate::presentation::people::students::controller::update,
        crate::presentation::academic::academic_years::controller::create,
        crate::presentation::academic::academic_years::controller::list,

        crate::presentation::analytics::controller::get_overview,
        crate::presentation::analytics::controller::get_dashboard,
        crate::presentation::academic::classes::controller::create,
        crate::presentation::academic::classes::controller::list,
        crate::presentation::academic::enrollments::controller::create,
        crate::presentation::academic::enrollments::controller::list,
        crate::presentation::dapodik::controller::list_sync_records,
        crate::presentation::dapodik::controller::list_outbox_jobs,
        crate::presentation::people::teacher::controller::create,
        crate::presentation::people::teacher::controller::list,
        crate::presentation::system::controller::list_tenants,
        crate::presentation::auth::controller::list_users,
    ),
    components(
        schemas(
            crate::presentation::auth::controller::AuthUserDto,
            crate::presentation::dapodik::controller::DapodikSyncRecordDto,
            crate::presentation::dapodik::controller::DapodikOutboxJobDto,
            crate::response::ApiErrorDetail,
            crate::response::ApiMeta,
            crate::response::PaginationMeta,
            crate::presentation::auth::dto::login_request::LoginRequest,
            crate::presentation::auth::dto::login_response::LoginResponse,
            crate::presentation::tenant::dto::provision_tenant_request::ProvisionTenantRequest,
            crate::presentation::tenant::dto::provision_tenant_response::ProvisionTenantResponse,
            crate::presentation::school::dto::school_response::SchoolResponse,
            crate::presentation::people::students::dto::create_student_request::CreateStudentRequest,
            crate::presentation::people::students::dto::update_student_request::UpdateStudentRequest,
            crate::presentation::people::students::dto::student_response::StudentResponse,
            crate::presentation::academic::academic_years::controller::CreateAcademicYearRequest,
            crate::presentation::academic::academic_years::controller::AcademicYearResponse,
            crate::presentation::academic::classes::controller::CreateClassRequest,
            crate::presentation::academic::classes::controller::ClassResponse,
            crate::presentation::academic::enrollments::controller::EnrollStudentRequest,
            crate::presentation::academic::enrollments::controller::EnrollmentResponse,
            crate::presentation::people::teacher::dto::create_teacher_request::CreateTeacherRequest,
            crate::presentation::people::teacher::dto::teacher_response::TeacherResponse,

            crate::presentation::analytics::controller::AnalyticsOverviewResponse,
            crate::presentation::system::dto::system_responses::TenantSummaryResponse,
            crate::presentation::system::dto::system_responses::ActivateMasterResponse,
            crate::presentation::system::dto::activate_master_request::ActivateMasterRequest,
        )
    ),
    tags(
        (name = "Auth", description = "Authentication and User Session API"),
        (name = "Analytics", description = "Analytics and Dashboard API"),
        (name = "Tenant", description = "Multi-tenant Management API"),
        (name = "School", description = "School Domain API"),
        (name = "Student", description = "Student Domain API"),
        (name = "Teacher", description = "Teacher Domain API")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

pub struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.as_mut().unwrap();
        components.add_security_scheme(
            "Bearer",
            utoipa::openapi::security::SecurityScheme::Http(
                utoipa::openapi::security::HttpBuilder::new()
                    .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        )
    }
}
