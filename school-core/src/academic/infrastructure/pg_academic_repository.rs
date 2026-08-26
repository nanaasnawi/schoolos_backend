use crate::academic::domain::{
    academic_year::AcademicYear, class::Class, enrollment::Enrollment, grade_level::GradeLevel,
    subject::Subject, term::Term,
};
use crate::academic::infrastructure::repository_traits::{
    AcademicYearRepository, ClassRepository, EnrollmentRepository, GradeLevelRepository,
    SubjectRepository, TermRepository,
};
use crate::common::error::InfrastructureError;
use crate::common::models::page::Page;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgAcademicRepository {
    pool: PgPool,
}

impl PgAcademicRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AcademicYearRepository for PgAcademicRepository {
    async fn create(&self, year: &AcademicYear) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO academic_years (id, tenant_id, name, start_date, end_date, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(year.id)
        .bind(year.tenant_id)
        .bind(&year.name)
        .bind(year.start_date)
        .bind(year.end_date)
        .bind(year.is_active)
        .bind(year.created_at)
        .bind(year.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_active(
        &self,
        tenant_id: Uuid,
    ) -> Result<Option<AcademicYear>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, name, start_date, end_date, is_active, created_at, updated_at, deleted_at, deleted_by 
               FROM academic_years 
               WHERE tenant_id = $1 AND is_active = true AND deleted_at IS NULL 
               ORDER BY created_at DESC LIMIT 1"#
        )
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| AcademicYear {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            name: r.get("name"),
            start_date: r.get("start_date"),
            end_date: r.get("end_date"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            deleted_by: r.get("deleted_by"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<AcademicYear>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, name, start_date, end_date, is_active, created_at, updated_at, deleted_at, deleted_by 
               FROM academic_years 
               WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| AcademicYear {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            name: r.get("name"),
            start_date: r.get("start_date"),
            end_date: r.get("end_date"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            deleted_by: r.get("deleted_by"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }
}

#[async_trait]
impl ClassRepository for PgAcademicRepository {
    async fn create(&self, class: &Class) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO classes (id, tenant_id, academic_year_id, grade_level_id, homeroom_teacher_id, name, capacity, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#
        )
        .bind(class.id)
        .bind(class.tenant_id)
        .bind(class.academic_year_id)
        .bind(class.grade_level_id)
        .bind(class.homeroom_teacher_id)
        .bind(&class.name)
        .bind(class.capacity)
        .bind(class.created_at)
        .bind(class.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Class>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, academic_year_id, grade_level_id, homeroom_teacher_id, name, capacity, created_at, updated_at, deleted_at, deleted_by 
               FROM classes WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| Class {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            academic_year_id: r.get("academic_year_id"),
            grade_level_id: r.get("grade_level_id"),
            homeroom_teacher_id: r.get("homeroom_teacher_id"),
            name: r.get("name"),
            capacity: r.get("capacity"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            deleted_by: r.get("deleted_by"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }

    async fn list(
        &self,
        tenant_id: Uuid,
        academic_year_id: Option<Uuid>,
        page: u64,
        page_size: u64,
    ) -> Result<Page<Class>, InfrastructureError> {
        let offset = (page.saturating_sub(1)) * page_size;
        
        let (records, count_row) = if let Some(ay_id) = academic_year_id {
            let records = sqlx::query(
                r#"SELECT id, tenant_id, academic_year_id, grade_level_id, homeroom_teacher_id, name, capacity, created_at, updated_at, deleted_at, deleted_by
                   FROM classes WHERE tenant_id = $1 AND academic_year_id = $2 AND deleted_at IS NULL
                   ORDER BY name ASC
                   LIMIT $3 OFFSET $4"#,
            )
            .bind(tenant_id)
            .bind(ay_id)
            .bind(page_size as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

            let count_row = sqlx::query_scalar::<_, i64>(
                r#"SELECT COUNT(*) FROM classes WHERE tenant_id = $1 AND academic_year_id = $2 AND deleted_at IS NULL"#,
            )
            .bind(tenant_id)
            .bind(ay_id)
            .fetch_one(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;
            
            (records, count_row)
        } else {
            let records = sqlx::query(
                r#"SELECT id, tenant_id, academic_year_id, grade_level_id, homeroom_teacher_id, name, capacity, created_at, updated_at, deleted_at, deleted_by
                   FROM classes WHERE tenant_id = $1 AND deleted_at IS NULL
                   ORDER BY name ASC
                   LIMIT $2 OFFSET $3"#,
            )
            .bind(tenant_id)
            .bind(page_size as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

            let count_row = sqlx::query_scalar::<_, i64>(
                r#"SELECT COUNT(*) FROM classes WHERE tenant_id = $1 AND deleted_at IS NULL"#,
            )
            .bind(tenant_id)
            .fetch_one(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;
            
            (records, count_row)
        };

        let items = records
            .into_iter()
            .map(|r| Class {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                academic_year_id: r.get("academic_year_id"),
                grade_level_id: r.get("grade_level_id"),
                homeroom_teacher_id: r.get("homeroom_teacher_id"),
                name: r.get("name"),
                capacity: r.get("capacity"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(Page::new(items, count_row, page, page_size))
    }
}

#[async_trait]
impl EnrollmentRepository for PgAcademicRepository {
    async fn create(&self, enrollment: &Enrollment) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO enrollments (id, tenant_id, student_id, class_id, academic_year_id, status, enrolled_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(enrollment.id)
        .bind(enrollment.tenant_id)
        .bind(enrollment.student_id)
        .bind(enrollment.class_id)
        .bind(enrollment.academic_year_id)
        .bind(&enrollment.status)
        .bind(enrollment.enrolled_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }
}

#[async_trait]
impl SubjectRepository for PgAcademicRepository {
    async fn create(&self, subject: &Subject) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO subjects (id, tenant_id, code, name, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(subject.id)
        .bind(subject.tenant_id)
        .bind(&subject.code)
        .bind(&subject.name)
        .bind(subject.is_active)
        .bind(subject.created_at)
        .bind(subject.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn update(&self, subject: &Subject) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE subjects
            SET code = $1, name = $2, is_active = $3, updated_at = $4
            WHERE id = $5 AND deleted_at IS NULL
            "#,
        )
        .bind(&subject.code)
        .bind(&subject.name)
        .bind(subject.is_active)
        .bind(subject.updated_at)
        .bind(subject.id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Subject>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, code, name, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM subjects WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| Subject {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            code: r.get("code"),
            name: r.get("name"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            deleted_by: r.get("deleted_by"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Subject>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, code, name, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM subjects WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY code ASC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let subjects = records
            .into_iter()
            .map(|r| Subject {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                code: r.get("code"),
                name: r.get("name"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(subjects)
    }
}

#[async_trait]
impl TermRepository for PgAcademicRepository {
    async fn create(&self, term: &Term) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO terms (id, academic_year_id, name, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(term.id)
        .bind(term.academic_year_id)
        .bind(&term.name)
        .bind(term.is_active)
        .bind(term.created_at)
        .bind(term.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn update(&self, term: &Term) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE terms
            SET name = $1, is_active = $2, updated_at = $3
            WHERE id = $4 AND deleted_at IS NULL
            "#,
        )
        .bind(&term.name)
        .bind(term.is_active)
        .bind(term.updated_at)
        .bind(term.id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Term>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, academic_year_id, name, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM terms WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| Term {
            id: r.get("id"),
            academic_year_id: r.get("academic_year_id"),
            name: r.get("name"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            deleted_by: r.get("deleted_by"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }

    async fn find_by_academic_year(
        &self,
        academic_year_id: Uuid,
    ) -> Result<Vec<Term>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, academic_year_id, name, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM terms WHERE academic_year_id = $1 AND deleted_at IS NULL
               ORDER BY created_at ASC"#
        )
        .bind(academic_year_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let terms = records
            .into_iter()
            .map(|r| Term {
                id: r.get("id"),
                academic_year_id: r.get("academic_year_id"),
                name: r.get("name"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(terms)
    }
}

#[async_trait]
impl GradeLevelRepository for PgAcademicRepository {
    async fn create(&self, grade_level: &GradeLevel) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO grade_levels (id, tenant_id, level, name, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(grade_level.id)
        .bind(grade_level.tenant_id)
        .bind(grade_level.level)
        .bind(&grade_level.name)
        .bind(grade_level.created_at)
        .bind(grade_level.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<GradeLevel>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, level, name, created_at, updated_at, deleted_at, deleted_by
               FROM grade_levels WHERE id = $1 AND deleted_at IS NULL"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| GradeLevel {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            level: r.get("level"),
            name: r.get("name"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
            deleted_by: r.get("deleted_by"),
            domain_events: Vec::new(),
            version: 1,
        }))
    }

    async fn find_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<GradeLevel>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, level, name, created_at, updated_at, deleted_at, deleted_by
               FROM grade_levels WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY level ASC"#,
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| GradeLevel {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                level: r.get("level"),
                name: r.get("name"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(items)
    }

    async fn list(
        &self,
        tenant_id: Uuid,
        page: u64,
        page_size: u64,
    ) -> Result<Page<GradeLevel>, InfrastructureError> {
        let offset = (page.saturating_sub(1)) * page_size;
        let records = sqlx::query(
            r#"SELECT id, tenant_id, level, name, created_at, updated_at, deleted_at, deleted_by
               FROM grade_levels WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY level ASC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(tenant_id)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let count_row = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM grade_levels WHERE tenant_id = $1 AND deleted_at IS NULL"#,
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| GradeLevel {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                level: r.get("level"),
                name: r.get("name"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            })
            .collect();

        Ok(Page::new(items, count_row, page, page_size))
    }
}
