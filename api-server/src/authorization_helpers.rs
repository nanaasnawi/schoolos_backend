use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{error::ApiError, extractors::RequestContext};
use school_core::common::error::ApplicationError;
use school_core::common::error_code::ErrorCode;

pub struct AuthorizationScope;

impl AuthorizationScope {
    /// Resolve the students.id from an authenticated actor's user_id within a tenant
    pub async fn resolve_student_id(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id FROM students WHERE user_id = $1 AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1",
        )
        .bind(user_id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.and_then(|r| r.try_get("id").ok()))
    }

    /// Resolve the teachers.id from an authenticated actor's user_id within a tenant
    pub async fn resolve_teacher_id(
        pool: &PgPool,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id FROM teachers WHERE user_id = $1 AND tenant_id = $2 AND deleted_at IS NULL LIMIT 1",
        )
        .bind(user_id)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.and_then(|r| r.try_get("id").ok()))
    }

    /// Check if a student is actively enrolled in a class
    pub async fn is_student_enrolled_in_class(
        pool: &PgPool,
        tenant_id: Uuid,
        student_id: Uuid,
        class_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM students
                WHERE tenant_id = $1 
                  AND id = $2 
                  AND class_id = $3
                  AND deleted_at IS NULL
                UNION
                SELECT 1 FROM enrollments 
                WHERE tenant_id = $1 
                  AND student_id = $2 
                  AND class_id = $3 
                  AND (status ILIKE 'active')
            ) as is_enrolled
            "#,
        )
        .bind(tenant_id)
        .bind(student_id)
        .bind(class_id)
        .fetch_one(pool)
        .await?;

        Ok(row.try_get("is_enrolled").unwrap_or(false))
    }

    /// Check if a parent/guardian has an actively enrolled child in a class
    pub async fn is_parent_of_enrolled_student(
        pool: &PgPool,
        tenant_id: Uuid,
        parent_user_id: Uuid,
        class_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1 
                FROM guardians g
                JOIN students s ON s.guardian_id = g.id
                WHERE g.user_id = $1 
                  AND g.tenant_id = $2
                  AND s.class_id = $3
                  AND s.deleted_at IS NULL
                UNION
                SELECT 1 
                FROM guardians g
                JOIN students s ON s.guardian_id = g.id
                JOIN enrollments en ON en.student_id = s.id
                WHERE g.user_id = $1 
                  AND g.tenant_id = $2
                  AND en.class_id = $3
                  AND (en.status ILIKE 'active')
            ) as is_parent
            "#,
        )
        .bind(parent_user_id)
        .bind(tenant_id)
        .bind(class_id)
        .fetch_one(pool)
        .await?;

        Ok(row.try_get("is_parent").unwrap_or(false))
    }

    /// Check if a teacher is assigned to a class (homeroom or teaching schedule)
    pub async fn is_teacher_assigned_to_class(
        pool: &PgPool,
        tenant_id: Uuid,
        teacher_id: Uuid,
        class_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM classes 
                WHERE id = $1 AND tenant_id = $2 AND homeroom_teacher_id = $3
                UNION
                SELECT 1 FROM class_schedules 
                WHERE class_id = $1 AND tenant_id = $2 AND teacher_id = $3 AND deleted_at IS NULL
            ) as is_assigned
            "#,
        )
        .bind(class_id)
        .bind(tenant_id)
        .bind(teacher_id)
        .fetch_one(pool)
        .await?;

        Ok(row.try_get("is_assigned").unwrap_or(false))
    }

    /// Verifies access to a scoped learning resource (material, assignment, quiz, CBT).
    /// Enforces multi-tenant isolation, cross-class isolation, and role restrictions.
    pub async fn verify_learning_resource_access(
        pool: &PgPool,
        req_ctx: &RequestContext,
        resource_tenant_id: Uuid,
        resource_class_id: Option<Uuid>,
        resource_teacher_id: Option<Uuid>,
        resource_created_by: Option<Uuid>,
    ) -> Result<(), ApiError> {
        // 1. Strict Tenant Isolation
        if resource_tenant_id != req_ctx.tenant_id {
            return Err(ApiError::new(
                ApplicationError::Unauthorized(
                    ErrorCode::AuthPermissionDenied,
                    "Akses lintas tenant sekolah tidak diizinkan".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }

        let actor = match &req_ctx.actor {
            Some(a) => a,
            None => {
                return Err(ApiError::new(
                    ApplicationError::Unauthorized(
                        ErrorCode::AuthPermissionDenied,
                        "Autentikasi diperlukan untuk mengakses sumber daya pembelajaran".to_string(),
                    ),
                    &req_ctx.request_id,
                ));
            }
        };

        // 2. Admins, Principals, Staff have tenant-wide read access
        let is_admin = actor.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("admin")
                || n.contains("kepala")
                || n.contains("operator")
                || n.contains("staf")
                || n.contains("staff")
        });
        if is_admin {
            return Ok(());
        }

        let is_teacher = actor.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("guru") || n.contains("teacher") || n.contains("pengajar")
        }) || Self::resolve_teacher_id(pool, req_ctx.tenant_id, actor.id).await.ok().flatten().is_some();
        let is_parent = actor.roles.iter().any(|r| {
            let n = r.name.to_lowercase();
            n.contains("wali") || n.contains("parent") || n.contains("guardian") || n.contains("ortu")
        });
        let is_student = !is_parent && !is_teacher && (
            actor.roles.iter().any(|r| {
                let n = r.name.to_lowercase();
                n == "siswa" || n == "student" || n == "murid" || n.contains("siswa")
            }) || Self::resolve_student_id(pool, req_ctx.tenant_id, actor.id).await.ok().flatten().is_some()
        );

        // 3. Teacher Access Check (Strict Cross-Teacher Isolation)
        if is_teacher {
            // If creator, allow
            if resource_created_by == Some(actor.id) {
                return Ok(());
            }

            let teacher_id = Self::resolve_teacher_id(pool, req_ctx.tenant_id, actor.id)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            if let Some(tid) = teacher_id {
                // If resource assigned to this teacher
                if resource_teacher_id == Some(tid) {
                    return Ok(());
                }
            }

            return Err(ApiError::new(
                ApplicationError::Unauthorized(
                    ErrorCode::AuthPermissionDenied,
                    "Tugas atau materi ini milik guru lain. Anda hanya dapat mengakses tugas milik Anda sendiri.".to_string(),
                ),
                &req_ctx.request_id,
            ));
        }

        // 4. Student Access Check (Class Scope Isolation)
        if is_student {
            let student_id = Self::resolve_student_id(pool, req_ctx.tenant_id, actor.id)
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?
                .ok_or_else(|| {
                    ApiError::new(
                        ApplicationError::Unauthorized(
                            ErrorCode::AuthPermissionDenied,
                            "Profil siswa tidak ditemukan untuk akun ini".to_string(),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

            match resource_class_id {
                Some(cid) => {
                    let enrolled = Self::is_student_enrolled_in_class(
                        pool,
                        req_ctx.tenant_id,
                        student_id,
                        cid,
                    )
                    .await
                    .map_err(|e| {
                        ApiError::new(
                            ApplicationError::Infrastructure(
                                school_core::common::error::InfrastructureError::Database(e),
                            ),
                            &req_ctx.request_id,
                        )
                    })?;

                    if !enrolled {
                        return Err(ApiError::new(
                            ApplicationError::Unauthorized(
                                ErrorCode::AuthPermissionDenied,
                                "Materi/Tugas ini bukan untuk kelas Anda. Akses ditolak demi keamanan data kelas."
                                    .to_string(),
                            ),
                            &req_ctx.request_id,
                        ));
                    }
                }
                None => {
                    // Unscoped resources can be viewed if published tenant-wide
                }
            }

            return Ok(());
        }

        // 5. Parent Access Check
        if is_parent {
            if let Some(cid) = resource_class_id {
                let is_child_class = Self::is_parent_of_enrolled_student(
                    pool,
                    req_ctx.tenant_id,
                    actor.id,
                    cid,
                )
                .await
                .map_err(|e| {
                    ApiError::new(
                        ApplicationError::Infrastructure(
                            school_core::common::error::InfrastructureError::Database(e),
                        ),
                        &req_ctx.request_id,
                    )
                })?;

                if !is_child_class {
                    return Err(ApiError::new(
                        ApplicationError::Unauthorized(
                            ErrorCode::AuthPermissionDenied,
                            "Tugas/Materi ini tidak terkait dengan kelas putra/putri Anda".to_string(),
                        ),
                        &req_ctx.request_id,
                    ));
                }
            }
            return Ok(());
        }

        Ok(())
    }
}
