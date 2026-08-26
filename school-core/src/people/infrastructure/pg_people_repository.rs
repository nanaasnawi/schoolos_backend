use crate::common::error::InfrastructureError;
use crate::people::domain::{
    guardian::Guardian,
    read_models::{
        GuardianDetail, StaffDetail, StaffSummary, StudentDetail, StudentProfile, StudentSummary,
        TeacherDetail, TeacherSummary,
    },
    staff::Staff,
    student::{Student, StudentStatus},
    teacher::Teacher,
};
use crate::people::infrastructure::repository_traits::{
    StudentRepository, TeacherQueryRepository, TeacherRepository,
};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::common::infrastructure::uow::UnitOfWork;
use crate::common::models::page::Page;

pub struct PgPeopleRepository {
    pool: PgPool,
}

impl PgPeopleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn parse_status(row: &sqlx::postgres::PgRow) -> StudentStatus {
    let status_str: String = row.get("status");
    StudentStatus::from_db_str(&status_str)
}

// ─── TeacherRepository ────────────────────────────────────────────────────────

#[async_trait]
impl TeacherRepository for PgPeopleRepository {
    async fn create(
        &self,
        teacher: &Teacher,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<sqlx::Transaction<'static, sqlx::Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            INSERT INTO teachers (id, tenant_id, user_id, nip, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, subject, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
            "#
        )
        .bind(teacher.id)
        .bind(teacher.tenant_id)
        .bind(teacher.user_id)
        .bind(&teacher.nip)
        .bind(&teacher.full_name)
        .bind(&teacher.nuptk)
        .bind(&teacher.jk)
        .bind(&teacher.tempat_lahir)
        .bind(teacher.tanggal_lahir)
        .bind(&teacher.status_kepegawaian)
        .bind(&teacher.jenis_ptk)
        .bind(&teacher.agama)
        .bind(&teacher.alamat_jalan)
        .bind(&teacher.no_hp)
        .bind(&teacher.email)
        .bind(&teacher.subject)
        .bind(teacher.is_active)
        .bind(teacher.created_at)
        .bind(teacher.updated_at)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Teacher>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, nip, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, subject, is_active, created_at, updated_at, deleted_at, deleted_by FROM teachers WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| {
            Teacher::rehydrate(
                r.get("id"),
                r.get("tenant_id"),
                r.get("user_id"),
                r.get("nip"),
                r.get("full_name"),
                r.get("nuptk"),
                r.get("jk"),
                r.get("tempat_lahir"),
                r.get("tanggal_lahir"),
                r.get("status_kepegawaian"),
                r.get("jenis_ptk"),
                r.get("agama"),
                r.get("alamat_jalan"),
                r.get("no_hp"),
                r.get("email"),
                r.get("subject"),
                r.get("is_active"),
                r.get("created_at"),
                r.get("updated_at"),
                r.get("deleted_at"),
                r.get("deleted_by"),
            )
        }))
    }

    async fn update(
        &self,
        teacher: &Teacher,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<sqlx::Transaction<'static, sqlx::Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            UPDATE teachers
            SET user_id = $1, nip = $2, full_name = $3, nuptk = $4, jk = $5, tempat_lahir = $6, tanggal_lahir = $7, status_kepegawaian = $8, jenis_ptk = $9, agama = $10, alamat_jalan = $11, no_hp = $12, email = $13, subject = $14, is_active = $15, updated_at = $16
            WHERE id = $17 AND tenant_id = $18
            "#,
        )
        .bind(teacher.user_id)
        .bind(&teacher.nip)
        .bind(&teacher.full_name)
        .bind(&teacher.nuptk)
        .bind(&teacher.jk)
        .bind(&teacher.tempat_lahir)
        .bind(teacher.tanggal_lahir)
        .bind(&teacher.status_kepegawaian)
        .bind(&teacher.jenis_ptk)
        .bind(&teacher.agama)
        .bind(&teacher.alamat_jalan)
        .bind(&teacher.no_hp)
        .bind(&teacher.email)
        .bind(&teacher.subject)
        .bind(teacher.is_active)
        .bind(teacher.updated_at)
        .bind(teacher.id)
        .bind(teacher.tenant_id)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Teacher>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, user_id, nip, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, subject, is_active, created_at, updated_at, deleted_at, deleted_by FROM teachers WHERE tenant_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(records
            .into_iter()
            .map(|r| {
                Teacher::rehydrate(
                    r.get("id"),
                    r.get("tenant_id"),
                    r.get("user_id"),
                    r.get("nip"),
                    r.get("full_name"),
                    r.get("nuptk"),
                    r.get("jk"),
                    r.get("tempat_lahir"),
                    r.get("tanggal_lahir"),
                    r.get("status_kepegawaian"),
                    r.get("jenis_ptk"),
                    r.get("agama"),
                    r.get("alamat_jalan"),
                    r.get("no_hp"),
                    r.get("email"),
                    r.get("subject"),
                    r.get("is_active"),
                    r.get("created_at"),
                    r.get("updated_at"),
                    r.get("deleted_at"),
                    r.get("deleted_by"),
                )
            })
            .collect())
    }
}

// ─── TeacherQueryRepository ──────────────────────────────────────────────────

fn teacher_is_active_string(row: &sqlx::postgres::PgRow) -> String {
    let active: bool = row.get("is_active");
    if active {
        "Active".to_string()
    } else {
        "Inactive".to_string()
    }
}

#[async_trait]
impl TeacherQueryRepository for PgPeopleRepository {
    async fn search(
        &self,
        query: crate::people::application::teacher::list::ListTeachersQuery,
    ) -> Result<crate::common::models::page::Page<TeacherSummary>, InfrastructureError> {
        use sqlx::QueryBuilder;

        let base_where = "deleted_at IS NULL AND tenant_id = ";

        let mut count_qb: QueryBuilder<sqlx::Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM teachers WHERE ");
        count_qb.push(base_where);
        count_qb.push_bind(query.tenant_id);

        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT id, nip, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, subject, is_active, updated_at FROM teachers WHERE ",
        );
        qb.push(base_where);
        qb.push_bind(query.tenant_id);

        if let Some(search) = &query.filter.search {
            let cond = " AND full_name ILIKE '%' || ";
            count_qb.push(cond).push_bind(search).push(" || '%'");
            qb.push(cond).push_bind(search).push(" || '%'");
        }

        if let Some(status) = &query.filter.status {
            let is_active = status.eq_ignore_ascii_case("Active");
            let cond = " AND is_active = ";
            count_qb.push(cond).push_bind(is_active);
            qb.push(cond).push_bind(is_active);
        }

        if let Some(after) = query.filter.created_after {
            let cond = " AND created_at >= ";
            count_qb.push(cond).push_bind(after);
            qb.push(cond).push_bind(after);
        }

        if let Some(before) = query.filter.created_before {
            let cond = " AND created_at <= ";
            count_qb.push(cond).push_bind(before);
            qb.push(cond).push_bind(before);
        }

        let total: i64 = count_qb
            .build()
            .fetch_one(&self.pool)
            .await
            .map(|row| sqlx::Row::get(&row, 0))
            .map_err(InfrastructureError::Database)?;

        let sort_field = match query.sort.field.as_str() {
            "full_name" => "full_name",
            "updated_at" => "updated_at",
            _ => "created_at",
        };
        let sort_dir = match query.sort.direction {
            crate::people::application::teacher::list::SortDirection::Asc => "ASC",
            crate::people::application::teacher::list::SortDirection::Desc => "DESC",
        };
        qb.push(format!(" ORDER BY {} {}", sort_field, sort_dir));

        let limit = query.pagination.page_size.clamp(1, 100) as i64;
        let offset =
            ((query.pagination.page.saturating_sub(1)) * query.pagination.page_size) as i64;
        qb.push(" LIMIT ").push_bind(limit);
        qb.push(" OFFSET ").push_bind(offset);

        let records = qb
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

        let summaries: Vec<TeacherSummary> = records
            .into_iter()
            .map(|r| TeacherSummary {
                id: r.get("id"),
                nip: r.get("nip"),
                full_name: r.get("full_name"),
                nuptk: r.try_get("nuptk").unwrap_or(None),
                jk: r.try_get("jk").unwrap_or(None),
                tempat_lahir: r.try_get("tempat_lahir").unwrap_or(None),
                tanggal_lahir: r.try_get("tanggal_lahir").unwrap_or(None),
                status_kepegawaian: r.try_get("status_kepegawaian").unwrap_or(None),
                jenis_ptk: r.try_get("jenis_ptk").unwrap_or(None),
                agama: r.try_get("agama").unwrap_or(None),
                alamat_jalan: r.try_get("alamat_jalan").unwrap_or(None),
                no_hp: r.try_get("no_hp").unwrap_or(None),
                email: r.try_get("email").unwrap_or(None),
                subject: r.get("subject"),
                status: teacher_is_active_string(&r),
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(crate::common::models::page::Page::new(
            summaries,
            total,
            query.pagination.page,
            query.pagination.page_size,
        ))
    }

    async fn get_detail(&self, id: Uuid) -> Result<Option<TeacherDetail>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, nip, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, subject, is_active, created_at, updated_at, deleted_at FROM teachers WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| TeacherDetail {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            user_id: r.get("user_id"),
            nip: r.get("nip"),
            full_name: r.get("full_name"),
            nuptk: r.try_get("nuptk").unwrap_or(None),
            jk: r.try_get("jk").unwrap_or(None),
            tempat_lahir: r.try_get("tempat_lahir").unwrap_or(None),
            tanggal_lahir: r.try_get("tanggal_lahir").unwrap_or(None),
            status_kepegawaian: r.try_get("status_kepegawaian").unwrap_or(None),
            jenis_ptk: r.try_get("jenis_ptk").unwrap_or(None),
            agama: r.try_get("agama").unwrap_or(None),
            alamat_jalan: r.try_get("alamat_jalan").unwrap_or(None),
            no_hp: r.try_get("no_hp").unwrap_or(None),
            email: r.try_get("email").unwrap_or(None),
            subject: r.get("subject"),
            status: teacher_is_active_string(&r),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
        }))
    }
}

// ─── GuardianRepository ─────────────────────────────────────────────────────

#[async_trait]
impl crate::people::infrastructure::repository_traits::GuardianRepository for PgPeopleRepository {
    async fn create(
        &self,
        guardian: &Guardian,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<sqlx::Transaction<'static, sqlx::Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            INSERT INTO guardians (id, tenant_id, user_id, full_name, phone_number, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(guardian.id)
        .bind(guardian.tenant_id)
        .bind(guardian.user_id)
        .bind(&guardian.full_name)
        .bind(&guardian.phone_number)
        .bind(guardian.created_at)
        .bind(guardian.updated_at)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn update(
        &self,
        guardian: &Guardian,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<sqlx::Transaction<'static, sqlx::Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            UPDATE guardians
            SET full_name = $1, phone_number = $2, updated_at = $3
            WHERE id = $4 AND tenant_id = $5
            "#,
        )
        .bind(&guardian.full_name)
        .bind(&guardian.phone_number)
        .bind(guardian.updated_at)
        .bind(guardian.id)
        .bind(guardian.tenant_id)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Guardian>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, full_name, phone_number, created_at, updated_at, deleted_at, deleted_by FROM guardians WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| {
            Guardian::rehydrate(
                r.get("id"),
                r.get("tenant_id"),
                r.get("user_id"),
                r.get("full_name"),
                r.get("phone_number"),
                r.get("created_at"),
                r.get("updated_at"),
                r.get("deleted_at"),
                r.get("deleted_by"),
            )
        }))
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Guardian>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, user_id, full_name, phone_number, created_at, updated_at, deleted_at, deleted_by FROM guardians WHERE tenant_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(records
            .into_iter()
            .map(|r| {
                Guardian::rehydrate(
                    r.get("id"),
                    r.get("tenant_id"),
                    r.get("user_id"),
                    r.get("full_name"),
                    r.get("phone_number"),
                    r.get("created_at"),
                    r.get("updated_at"),
                    r.get("deleted_at"),
                    r.get("deleted_by"),
                )
            })
            .collect())
    }
}

// ─── GuardianQueryRepository ────────────────────────────────────────────────

#[async_trait]
impl crate::people::infrastructure::repository_traits::GuardianQueryRepository
    for PgPeopleRepository
{
    async fn search(
        &self,
        query: crate::people::application::guardian::list::ListGuardiansQuery,
    ) -> Result<Page<GuardianDetail>, InfrastructureError> {
        use sqlx::QueryBuilder;

        let base_where = "deleted_at IS NULL AND tenant_id = ";

        let mut count_qb: QueryBuilder<sqlx::Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM guardians WHERE ");
        count_qb.push(base_where);
        count_qb.push_bind(query.tenant_id);

        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT id, tenant_id, user_id, full_name, phone_number, created_at, updated_at, deleted_at FROM guardians WHERE "
        );
        qb.push(base_where);
        qb.push_bind(query.tenant_id);

        if let Some(search) = &query.filter.search {
            let cond = " AND full_name ILIKE '%' || ";
            count_qb.push(cond).push_bind(search).push(" || '%'");
            qb.push(cond).push_bind(search).push(" || '%'");
        }

        let total: i64 = count_qb
            .build()
            .fetch_one(&self.pool)
            .await
            .map(|row| sqlx::Row::get(&row, 0))
            .map_err(InfrastructureError::Database)?;

        let sort_dir = match query.sort.direction {
            crate::people::application::guardian::list::SortDirection::Asc => "ASC",
            crate::people::application::guardian::list::SortDirection::Desc => "DESC",
        };
        qb.push(format!(" ORDER BY created_at {}", sort_dir));

        let limit = query.pagination.page_size.clamp(1, 100) as i64;
        let offset =
            ((query.pagination.page.saturating_sub(1)) * query.pagination.page_size) as i64;
        qb.push(" LIMIT ").push_bind(limit);
        qb.push(" OFFSET ").push_bind(offset);

        let records = qb
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

        let items: Vec<GuardianDetail> = records
            .into_iter()
            .map(|r| GuardianDetail {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                user_id: r.get("user_id"),
                full_name: r.get("full_name"),
                phone_number: r.get("phone_number"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
            })
            .collect();

        Ok(Page::new(
            items,
            total,
            query.pagination.page,
            query.pagination.page_size,
        ))
    }

    async fn get_detail(&self, id: Uuid) -> Result<Option<GuardianDetail>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, full_name, phone_number, created_at, updated_at, deleted_at FROM guardians WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| GuardianDetail {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            user_id: r.get("user_id"),
            full_name: r.get("full_name"),
            phone_number: r.get("phone_number"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
        }))
    }
}

// ─── StaffRepository ─────────────────────────────────────────────────────────

#[async_trait]
impl crate::people::infrastructure::repository_traits::StaffRepository for PgPeopleRepository {
    async fn create(
        &self,
        staff: &Staff,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<sqlx::Transaction<'static, sqlx::Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            INSERT INTO staff (id, tenant_id, user_id, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, nip, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, job_title, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
            "#
        )
        .bind(staff.id)
        .bind(staff.tenant_id)
        .bind(staff.user_id)
        .bind(&staff.full_name)
        .bind(&staff.nuptk)
        .bind(&staff.jk)
        .bind(&staff.tempat_lahir)
        .bind(staff.tanggal_lahir)
        .bind(&staff.nip)
        .bind(&staff.status_kepegawaian)
        .bind(&staff.jenis_ptk)
        .bind(&staff.agama)
        .bind(&staff.alamat_jalan)
        .bind(&staff.no_hp)
        .bind(&staff.email)
        .bind(&staff.job_title)
        .bind(staff.is_active)
        .bind(staff.created_at)
        .bind(staff.updated_at)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn update(
        &self,
        staff: &Staff,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<sqlx::Transaction<'static, sqlx::Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            UPDATE staff
            SET full_name = $1, nuptk = $2, jk = $3, tempat_lahir = $4, tanggal_lahir = $5, nip = $6, status_kepegawaian = $7, jenis_ptk = $8, agama = $9, alamat_jalan = $10, no_hp = $11, email = $12, job_title = $13, is_active = $14, updated_at = $15
            WHERE id = $16 AND tenant_id = $17
            "#,
        )
        .bind(&staff.full_name)
        .bind(&staff.nuptk)
        .bind(&staff.jk)
        .bind(&staff.tempat_lahir)
        .bind(staff.tanggal_lahir)
        .bind(&staff.nip)
        .bind(&staff.status_kepegawaian)
        .bind(&staff.jenis_ptk)
        .bind(&staff.agama)
        .bind(&staff.alamat_jalan)
        .bind(&staff.no_hp)
        .bind(&staff.email)
        .bind(&staff.job_title)
        .bind(staff.is_active)
        .bind(staff.updated_at)
        .bind(staff.id)
        .bind(staff.tenant_id)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Staff>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, nip, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, job_title, is_active, created_at, updated_at, deleted_at, deleted_by FROM staff WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| {
            Staff::rehydrate(
                r.get("id"),
                r.get("tenant_id"),
                r.get("user_id"),
                r.get("full_name"),
                r.get("nuptk"),
                r.get("jk"),
                r.get("tempat_lahir"),
                r.get("tanggal_lahir"),
                r.get("nip"),
                r.get("status_kepegawaian"),
                r.get("jenis_ptk"),
                r.get("agama"),
                r.get("alamat_jalan"),
                r.get("no_hp"),
                r.get("email"),
                r.get("job_title"),
                r.get("is_active"),
                r.get("created_at"),
                r.get("updated_at"),
                r.get("deleted_at"),
                r.get("deleted_by"),
            )
        }))
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Staff>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, user_id, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, nip, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, job_title, is_active, created_at, updated_at, deleted_at, deleted_by FROM staff WHERE tenant_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(records
            .into_iter()
            .map(|r| {
                Staff::rehydrate(
                    r.get("id"),
                    r.get("tenant_id"),
                    r.get("user_id"),
                    r.get("full_name"),
                    r.get("nuptk"),
                    r.get("jk"),
                    r.get("tempat_lahir"),
                    r.get("tanggal_lahir"),
                    r.get("nip"),
                    r.get("status_kepegawaian"),
                    r.get("jenis_ptk"),
                    r.get("agama"),
                    r.get("alamat_jalan"),
                    r.get("no_hp"),
                    r.get("email"),
                    r.get("job_title"),
                    r.get("is_active"),
                    r.get("created_at"),
                    r.get("updated_at"),
                    r.get("deleted_at"),
                    r.get("deleted_by"),
                )
            })
            .collect())
    }
}

// ─── StaffQueryRepository ────────────────────────────────────────────────────

#[async_trait]
impl crate::people::infrastructure::repository_traits::StaffQueryRepository for PgPeopleRepository {
    async fn search(
        &self,
        query: crate::people::application::staff::list::ListStaffQuery,
    ) -> Result<Page<StaffSummary>, InfrastructureError> {
        use sqlx::QueryBuilder;

        let base_where = "deleted_at IS NULL AND tenant_id = ";

        let mut count_qb: QueryBuilder<sqlx::Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM staff WHERE ");
        count_qb.push(base_where);
        count_qb.push_bind(query.tenant_id);

        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT id, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, nip, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, job_title, is_active, updated_at FROM staff WHERE ",
        );
        qb.push(base_where);
        qb.push_bind(query.tenant_id);

        if let Some(search) = &query.filter.search {
            let cond = " AND (full_name ILIKE '%' || ";
            count_qb
                .push(cond)
                .push_bind(search)
                .push(" || '%' OR job_title ILIKE '%' || ")
                .push_bind(search)
                .push(" || '%')");
            qb.push(cond)
                .push_bind(search)
                .push(" || '%' OR job_title ILIKE '%' || ")
                .push_bind(search)
                .push(" || '%')");
        }

        if let Some(active) = query.filter.is_active {
            let cond = " AND is_active = ";
            count_qb.push(cond).push_bind(active);
            qb.push(cond).push_bind(active);
        }

        let total: i64 = count_qb
            .build()
            .fetch_one(&self.pool)
            .await
            .map(|row| sqlx::Row::get(&row, 0))
            .map_err(InfrastructureError::Database)?;

        let sort_dir = match query.sort.direction {
            crate::people::application::staff::list::SortDirection::Asc => "ASC",
            crate::people::application::staff::list::SortDirection::Desc => "DESC",
        };
        qb.push(format!(" ORDER BY updated_at {}", sort_dir));

        let limit = query.pagination.page_size.clamp(1, 100) as i64;
        let offset =
            ((query.pagination.page.saturating_sub(1)) * query.pagination.page_size) as i64;
        qb.push(" LIMIT ").push_bind(limit);
        qb.push(" OFFSET ").push_bind(offset);

        let records = qb
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

        let items: Vec<StaffSummary> = records
            .into_iter()
            .map(|r| StaffSummary {
                id: r.get("id"),
                full_name: r.get("full_name"),
                nuptk: r.try_get("nuptk").unwrap_or(None),
                jk: r.try_get("jk").unwrap_or(None),
                tempat_lahir: r.try_get("tempat_lahir").unwrap_or(None),
                tanggal_lahir: r.try_get("tanggal_lahir").unwrap_or(None),
                nip: r.try_get("nip").unwrap_or(None),
                status_kepegawaian: r.try_get("status_kepegawaian").unwrap_or(None),
                jenis_ptk: r.try_get("jenis_ptk").unwrap_or(None),
                agama: r.try_get("agama").unwrap_or(None),
                alamat_jalan: r.try_get("alamat_jalan").unwrap_or(None),
                no_hp: r.try_get("no_hp").unwrap_or(None),
                email: r.try_get("email").unwrap_or(None),
                job_title: r.get("job_title"),
                is_active: r.get("is_active"),
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(Page::new(
            items,
            total,
            query.pagination.page,
            query.pagination.page_size,
        ))
    }

    async fn get_detail(&self, id: Uuid) -> Result<Option<StaffDetail>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, full_name, nuptk, jk, tempat_lahir, tanggal_lahir, nip, status_kepegawaian, jenis_ptk, agama, alamat_jalan, no_hp, email, job_title, is_active, created_at, updated_at, deleted_at FROM staff WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| StaffDetail {
            id: r.get("id"),
            tenant_id: r.get("tenant_id"),
            user_id: r.get("user_id"),
            full_name: r.get("full_name"),
            nuptk: r.try_get("nuptk").unwrap_or(None),
            jk: r.try_get("jk").unwrap_or(None),
            tempat_lahir: r.try_get("tempat_lahir").unwrap_or(None),
            tanggal_lahir: r.try_get("tanggal_lahir").unwrap_or(None),
            nip: r.try_get("nip").unwrap_or(None),
            status_kepegawaian: r.try_get("status_kepegawaian").unwrap_or(None),
            jenis_ptk: r.try_get("jenis_ptk").unwrap_or(None),
            agama: r.try_get("agama").unwrap_or(None),
            alamat_jalan: r.try_get("alamat_jalan").unwrap_or(None),
            no_hp: r.try_get("no_hp").unwrap_or(None),
            email: r.try_get("email").unwrap_or(None),
            job_title: r.get("job_title"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            deleted_at: r.get("deleted_at"),
        }))
    }
}

// ─── StudentRepository (Command side) ────────────────────────────────────────

#[async_trait]
impl StudentRepository for PgPeopleRepository {
    async fn create(
        &self,
        student: &Student,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<sqlx::Transaction<'static, sqlx::Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            INSERT INTO students (id, tenant_id, user_id, guardian_id, nisn, full_name, nik, gender, place_of_birth, date_of_birth, religion, nipd, alamat_jalan, no_hp, email, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#
        )
        .bind(student.id)
        .bind(student.tenant_id)
        .bind(student.user_id)
        .bind(student.guardian_id)
        .bind(&student.nisn)
        .bind(&student.full_name)
        .bind(&student.nik)
        .bind(&student.gender)
        .bind(&student.place_of_birth)
        .bind(student.date_of_birth)
        .bind(&student.religion)
        .bind(&student.nipd)
        .bind(&student.alamat_jalan)
        .bind(&student.no_hp)
        .bind(&student.email)
        .bind(student.status.as_db_str())
        .bind(student.created_at)
        .bind(student.updated_at)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Student>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, guardian_id, nisn, full_name, nik, gender, place_of_birth, date_of_birth, religion, nipd, alamat_jalan, no_hp, email, status, created_at, updated_at, deleted_at, deleted_by FROM students WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| {
            Student::rehydrate(
                r.get("id"),
                r.get("tenant_id"),
                r.get("user_id"),
                r.get("nisn"),
                r.get("full_name"),
                r.get("nik"),
                r.get("gender"),
                r.get("place_of_birth"),
                r.get("date_of_birth"),
                r.get("religion"),
                r.get("nipd"),
                r.get("alamat_jalan"),
                r.get("no_hp"),
                r.get("email"),
                r.get("guardian_id"),
                parse_status(&r), // ✅ stable deserialization
                r.get("created_at"),
                r.get("updated_at"),
                r.get("deleted_at"),
                r.get("deleted_by"),
            )
        }))
    }

    async fn update(
        &self,
        student: &Student,
        uow: &mut dyn UnitOfWork,
    ) -> Result<(), InfrastructureError> {
        let tx = uow
            .as_any()
            .downcast_mut::<sqlx::Transaction<'static, sqlx::Postgres>>()
            .ok_or_else(|| InfrastructureError::Internal("Expected sqlx::Transaction".into()))?;

        sqlx::query(
            r#"
            UPDATE students
            SET nisn = $1, full_name = $2, nik = $3, gender = $4, place_of_birth = $5, date_of_birth = $6, religion = $7, nipd = $8, alamat_jalan = $9, no_hp = $10, email = $11, guardian_id = $12, status = $13, updated_at = $14
            WHERE id = $15 AND tenant_id = $16
            "#,
        )
        .bind(&student.nisn)
        .bind(&student.full_name)
        .bind(&student.nik)
        .bind(&student.gender)
        .bind(&student.place_of_birth)
        .bind(student.date_of_birth)
        .bind(&student.religion)
        .bind(&student.nipd)
        .bind(&student.alamat_jalan)
        .bind(&student.no_hp)
        .bind(&student.email)
        .bind(student.guardian_id)
        .bind(student.status.as_db_str()) // ✅ stable
        .bind(student.updated_at)
        .bind(student.id)
        .bind(student.tenant_id)
        .execute(&mut **tx)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(())
    }

    async fn find_by_tenant(&self, tenant_id: Uuid) -> Result<Vec<Student>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, user_id, guardian_id, nisn, full_name, nik, gender, place_of_birth, date_of_birth, religion, nipd, alamat_jalan, no_hp, email, status, created_at, updated_at, deleted_at, deleted_by FROM students WHERE tenant_id = $1 AND deleted_at IS NULL ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(records
            .into_iter()
            .map(|r| {
                Student::rehydrate(
                    r.get("id"),
                    r.get("tenant_id"),
                    r.get("user_id"),
                    r.get("nisn"),
                    r.get("full_name"),
                    r.get("nik"),
                    r.get("gender"),
                    r.get("place_of_birth"),
                    r.get("date_of_birth"),
                    r.get("religion"),
                    r.get("nipd"),
                    r.get("alamat_jalan"),
                    r.get("no_hp"),
                    r.get("email"),
                    r.get("guardian_id"),
                    parse_status(&r),
                    r.get("created_at"),
                    r.get("updated_at"),
                    r.get("deleted_at"),
                    r.get("deleted_by"),
                )
            })
            .collect())
    }
}

// ─── StudentQueryRepository (Read side — CQRS) ────────────────────────────────

use crate::common::infrastructure::specification::Specification;

struct StudentFilterSpecification<'a> {
    filter: &'a crate::people::application::list_students::query::StudentFilter,
}

impl<'a> Specification<'a> for StudentFilterSpecification<'a> {
    fn apply_where(&'a self, builder: &mut sqlx::QueryBuilder<'a, sqlx::Postgres>) {
        if let Some(search) = &self.filter.search {
            builder.push(" AND students.full_name ILIKE '%' || ");
            builder.push_bind(search);
            builder.push(" || '%'");
        }

        if let Some(status) = &self.filter.status {
            builder.push(" AND students.status = ").push_bind(status.as_db_str());
        }

        if let Some(after) = self.filter.created_after {
            builder.push(" AND students.created_at >= ").push_bind(after);
        }

        if let Some(before) = self.filter.created_before {
            builder.push(" AND students.created_at <= ").push_bind(before);
        }

        if let Some(after) = self.filter.updated_after {
            builder.push(" AND students.updated_at >= ").push_bind(after);
        }

        if let Some(before) = self.filter.updated_before {
            builder.push(" AND students.updated_at <= ").push_bind(before);
        }
    }
}

#[async_trait]
impl crate::people::infrastructure::repository_traits::StudentQueryRepository
    for PgPeopleRepository
{
    async fn search(
        &self,
        query: crate::people::application::list_students::query::ListStudentsQuery,
    ) -> Result<crate::common::models::page::Page<StudentSummary>, InfrastructureError> {
        use sqlx::QueryBuilder;

        let base_where = "students.deleted_at IS NULL AND students.tenant_id = ";

        let mut count_qb: QueryBuilder<sqlx::Postgres> =
            QueryBuilder::new("SELECT COUNT(students.id) FROM students WHERE ");
        count_qb.push(base_where);
        count_qb.push_bind(query.tenant_id);

        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT students.id, students.nisn, students.full_name, students.nik, students.gender, students.place_of_birth, students.date_of_birth, students.religion, students.nipd, students.alamat_jalan, students.no_hp, students.email, students.status, students.updated_at, classes.name as class_name FROM students LEFT JOIN enrollments ON enrollments.student_id = students.id AND enrollments.status = 'Active' LEFT JOIN classes ON classes.id = enrollments.class_id WHERE ",
        );
        qb.push(base_where);
        qb.push_bind(query.tenant_id);

        // ── Filters via Specification ────────────────────────────────────────
        let spec = StudentFilterSpecification {
            filter: &query.filter,
        };

        spec.apply_joins(&mut count_qb);
        spec.apply_where(&mut count_qb);

        spec.apply_joins(&mut qb);
        spec.apply_where(&mut qb);

        // ── Count ────────────────────────────────────────────────────────────
        let total: i64 = count_qb
            .build()
            .fetch_one(&self.pool)
            .await
            .map(|row| sqlx::Row::get(&row, 0))
            .map_err(InfrastructureError::Database)?;

        // ── Sort & Paginate ──────────────────────────────────────────────────
        let sort_field = match query.sort.field.as_str() {
            "full_name" => "students.full_name",
            "updated_at" => "students.updated_at",
            _ => "students.created_at",
        };
        let sort_dir = match query.sort.direction {
            crate::people::application::list_students::query::SortDirection::Asc => "ASC",
            crate::people::application::list_students::query::SortDirection::Desc => "DESC",
        };
        qb.push(format!(" ORDER BY {} {}", sort_field, sort_dir));

        let limit = query.pagination.page_size.clamp(1, 2000) as i64;
        let offset =
            ((query.pagination.page.saturating_sub(1)) * query.pagination.page_size) as i64;
        qb.push(" LIMIT ").push_bind(limit);
        qb.push(" OFFSET ").push_bind(offset);

        let records = qb
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

        let summaries: Vec<StudentSummary> = records
            .into_iter()
            .map(|r| StudentSummary {
                id: r.get("id"),
                nisn: r.get("nisn"),
                full_name: r.get("full_name"),
                nik: r.try_get("nik").unwrap_or(None),
                gender: r.try_get("gender").unwrap_or(None),
                place_of_birth: r.try_get("place_of_birth").unwrap_or(None),
                date_of_birth: r.try_get("date_of_birth").unwrap_or(None),
                religion: r.try_get("religion").unwrap_or(None),
                nipd: r.try_get("nipd").unwrap_or(None),
                alamat_jalan: r.try_get("alamat_jalan").unwrap_or(None),
                no_hp: r.try_get("no_hp").unwrap_or(None),
                email: r.try_get("email").unwrap_or(None),
                status: parse_status(&r),
                class_name: r.try_get("class_name").unwrap_or(None),
                grade: None,
                updated_at: r.get("updated_at"),
            })
            .collect();

        Ok(crate::common::models::page::Page::new(
            summaries,
            total,
            query.pagination.page,
            query.pagination.page_size,
        ))
    }

    async fn get_profile(&self, id: Uuid) -> Result<Option<StudentProfile>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, user_id, guardian_id, nisn, full_name, nik, gender, place_of_birth, date_of_birth, religion, nipd, alamat_jalan, no_hp, email, status, created_at, updated_at FROM students WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        Ok(record.map(|r| {
            let detail = StudentDetail {
                id: r.get("id"),
                tenant_id: r.get("tenant_id"),
                user_id: r.get("user_id"),
                guardian_id: r.get("guardian_id"),
                nisn: r.get("nisn"),
                full_name: r.get("full_name"),
                nik: r.try_get("nik").unwrap_or(None),
                gender: r.try_get("gender").unwrap_or(None),
                place_of_birth: r.try_get("place_of_birth").unwrap_or(None),
                date_of_birth: r.try_get("date_of_birth").unwrap_or(None),
                religion: r.try_get("religion").unwrap_or(None),
                nipd: r.try_get("nipd").unwrap_or(None),
                alamat_jalan: r.try_get("alamat_jalan").unwrap_or(None),
                no_hp: r.try_get("no_hp").unwrap_or(None),
                email: r.try_get("email").unwrap_or(None),
                status: parse_status(&r),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            };

            StudentProfile {
                student: detail,
                guardian: None,      // Populated in Sprint 4.3 (Guardian API)
                current_class: None, // Populated in Sprint 4.3 (Academic API)
                current_enrollment: None,
                academic_year: None,
                attendance_summary: None,
                latest_assessment_summary: None,
            }
        }))
    }
}
