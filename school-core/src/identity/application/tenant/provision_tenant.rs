use crate::common::domain::clock::Clock;
use crate::common::error::ApplicationError;
use crate::identity::domain::school::School;
use crate::identity::domain::tenant::Tenant;
use crate::permission::domain::role::Role;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct ProvisionTenantCommand {
    pub tenant_name: String,
    pub school_name: String,
}

pub struct ProvisionTenantUseCase {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl ProvisionTenantUseCase {
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    pub async fn execute(&self, command: ProvisionTenantCommand) -> Result<Uuid, ApplicationError> {
        let seed_version = "1.0.0".to_string();

        let mut tx = self.pool.begin().await.map_err(|e| {
            ApplicationError::Infrastructure(crate::common::error::InfrastructureError::Database(e))
        })?;
        // 1. Create Tenant
        let tenant = Tenant::new(command.tenant_name, None, &*self.clock);
        sqlx::query(
            "INSERT INTO tenants (id, name, created_at, updated_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(tenant.id)
        .bind(&tenant.name)
        .bind(tenant.created_at)
        .bind(tenant.updated_at)
        .execute(&mut *tx)
        .await?;

        // 2. Insert Seed Version
        sqlx::query(
            "INSERT INTO tenant_seeds (id, tenant_id, version, seeded_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::now_v7())
        .bind(tenant.id)
        .bind(seed_version)
        .bind(self.clock.now())
        .execute(&mut *tx)
        .await?;

        // 3. Create School
        let school = School::new(tenant.id, command.school_name, &*self.clock);
        sqlx::query(
                    "INSERT INTO schools (id, tenant_id, name, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)"
                )
                    .bind(school.id)
                    .bind(school.tenant_id)
                    .bind(&school.name)
                    .bind(school.created_at)
                    .bind(school.updated_at)
                    .execute(&mut *tx)
                    .await?;

        // 4. Create Standard System Roles & Scope
        let standard_roles = vec![
            ("Kepala Sekolah", "Kepala Sekolah / Pimpinan Unit", "WEB, ANDROID"),
            ("Guru", "Guru Pengajar & Wali Kelas", "WEB, ANDROID"),
            ("Operator/Staff", "Operator Sekolah & Staf Administrasi", "WEB"),
            ("Bendahara", "Bendahara & Pengelola Keuangan Sekolah", "WEB, ANDROID"),
            ("Siswa", "Peserta Didik / Siswa", "ANDROID"),
            ("Wali Siswa", "Orang Tua / Wali Murid", "ANDROID"),
        ];

        for (role_name, desc, platforms) in standard_roles {
            let role_obj = Role::new(tenant.id, role_name.to_string(), true, &*self.clock);
            sqlx::query("INSERT INTO roles (id, tenant_id, name, description, allowed_platforms, is_system_default, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (tenant_id, name) DO NOTHING")
                .bind(role_obj.id).bind(tenant.id).bind(role_name).bind(desc).bind(platforms).bind(true).bind(role_obj.created_at).bind(role_obj.updated_at)
                .execute(&mut *tx).await?;
        }

        tx.commit().await.map_err(|e| {
            ApplicationError::Infrastructure(crate::common::error::InfrastructureError::Database(e))
        })?;

        Ok(tenant.id)
    }
}
