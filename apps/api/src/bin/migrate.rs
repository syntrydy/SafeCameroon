//! Applies pending migrations to `DATABASE_URL` and exits
//! (docs/DEPLOYMENT.md's deploy strategy: "build -> unit tests -> integration
//! tests -> migration validation -> deploy API -> deploy workers -> smoke
//! tests"). Forward-only, matching migrations/README.md -- this never runs a
//! `.down.sql` file. Run before deploying a new `safe-cameroon-api`/
//! `safe-cameroon-worker` build against a database, never automatically on
//! every app boot, so a migration is always a deliberate, reviewable step.

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("database connection must succeed");

    MIGRATOR.run(&pool).await.expect("migrations must apply");
    tracing::info!("migrations applied successfully");
}
