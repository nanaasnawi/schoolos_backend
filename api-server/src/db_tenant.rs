use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub use school_core::common::tenant_context::{
    apply_scope_to_pg_conn, current_tenant_db_scope, with_system_rls_bypass, with_tenant_db_scope,
    TenantDbScope,
};

/// Begins a transaction and locks PostgreSQL session-local RLS strictly to `tenant_id`.
pub async fn begin_tenant_tx<'a>(
    pool: &'a PgPool,
    tenant_id: Uuid,
) -> Result<Transaction<'a, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    set_tenant_context_on_tx(&mut tx, tenant_id).await?;
    Ok(tx)
}

/// Sets PostgreSQL transaction-local (`is_local = true`) RLS context on an existing transaction.
pub async fn set_tenant_context_on_tx(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "SELECT set_config('app.current_tenant_id', $1, true), set_config('app.bypass_rls', 'off', true)",
    )
    .bind(tenant_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Begins a transaction with explicit transaction-local system RLS bypass for cross-tenant maintenance or auth lookups.
pub async fn begin_system_tx<'a>(pool: &'a PgPool) -> Result<Transaction<'a, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "SELECT set_config('app.current_tenant_id', '', true), set_config('app.bypass_rls', 'on', true)",
    )
    .execute(&mut *tx)
    .await?;
    Ok(tx)
}
