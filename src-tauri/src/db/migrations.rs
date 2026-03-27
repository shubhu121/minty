use anyhow::Result;
use sqlx::SqlitePool;

/// Run all pending SQLite migrations from the `migrations/` directory.
/// Uses sqlx's compile-time checked migrations.
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
