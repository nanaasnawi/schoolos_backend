use sqlx::PgConnection;
use uuid::Uuid;

/// Active database RLS isolation scope carried across async execution on the current Tokio task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantDbScope {
    Tenant(Uuid),
    SystemBypass,
}

tokio::task_local! {
    static ACTIVE_TENANT_SCOPE: TenantDbScope;
}

/// Returns the active database tenant isolation scope for the current Tokio task, if set.
pub fn current_tenant_db_scope() -> Option<TenantDbScope> {
    ACTIVE_TENANT_SCOPE.try_with(|s| *s).ok()
}

/// Executes an async block with PostgreSQL RLS locked strictly to `tenant_id`.
pub async fn with_tenant_db_scope<F, T>(tenant_id: Uuid, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    ACTIVE_TENANT_SCOPE
        .scope(TenantDbScope::Tenant(tenant_id), fut)
        .await
}

/// Executes an async block with explicit platform/auth RLS bypass enabled.
pub async fn with_system_rls_bypass<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    ACTIVE_TENANT_SCOPE
        .scope(TenantDbScope::SystemBypass, fut)
        .await
}

/// Applies the given tenant scope (or resets to fail-closed when `None`) onto a PostgreSQL connection.
pub async fn apply_scope_to_pg_conn(
    conn: &mut PgConnection,
    scope: Option<TenantDbScope>,
) -> Result<(), sqlx::Error> {
    let (tenant_val, bypass_val): (String, &'static str) = match scope {
        Some(TenantDbScope::Tenant(tid)) => (tid.to_string(), "off"),
        Some(TenantDbScope::SystemBypass) => (String::new(), "on"),
        None => (String::new(), "off"),
    };

    sqlx::query(
        "SELECT set_config('app.current_tenant_id', $1, false), set_config('app.bypass_rls', $2, false)",
    )
    .bind(tenant_val)
    .bind(bypass_val)
    .execute(conn)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tenant_scope_isolated_across_concurrent_tasks() {
        let t1 = Uuid::now_v7();
        let t2 = Uuid::now_v7();

        assert_eq!(current_tenant_db_scope(), None);

        let (r1, r2, r3) = tokio::join!(
            with_tenant_db_scope(t1, async {
                tokio::task::yield_now().await;
                current_tenant_db_scope()
            }),
            with_tenant_db_scope(t2, async {
                tokio::task::yield_now().await;
                current_tenant_db_scope()
            }),
            with_system_rls_bypass(async { current_tenant_db_scope() }),
        );

        assert_eq!(r1, Some(TenantDbScope::Tenant(t1)));
        assert_eq!(r2, Some(TenantDbScope::Tenant(t2)));
        assert_eq!(r3, Some(TenantDbScope::SystemBypass));
        assert_eq!(current_tenant_db_scope(), None);
    }
}
