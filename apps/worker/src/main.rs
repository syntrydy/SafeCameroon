mod dispatch;

use std::sync::Arc;
use std::time::Duration;

use safe_cameroon_application::channel::ChannelRegistry;
use safe_cameroon_infrastructure::channels::{EmailChannel, SmsChannel, WhatsAppChannel};
use safe_cameroon_infrastructure::postgres::{PostgresAlertRepository, PostgresDeliveryRepository};
use sqlx::postgres::PgPoolOptions;

const BATCH_SIZE: i64 = 10;
const POLL_INTERVAL: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("database connection must succeed");

    let deliveries = PostgresDeliveryRepository::new(pool.clone());
    let alerts = PostgresAlertRepository::new(pool);

    // Mock/sandbox adapters: real vendor credentials are not available yet
    // (prompt 08). Endpoint validation and provider-error mapping are real;
    // only the actual network call is simulated.
    let mut registry = ChannelRegistry::new();
    registry.register(Arc::new(WhatsAppChannel));
    registry.register(Arc::new(SmsChannel));
    registry.register(Arc::new(EmailChannel));

    tracing::info!(poll_interval = ?POLL_INTERVAL, "safe-cameroon-worker: polling for deliveries");
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("safe-cameroon-worker: shutting down");
                break;
            }
            result = dispatch::process_batch(&deliveries, &alerts, &registry, BATCH_SIZE) => {
                match result {
                    Ok(0) => tokio::time::sleep(POLL_INTERVAL).await,
                    Ok(count) => tracing::info!(count, "safe-cameroon-worker: processed delivery job(s)"),
                    Err(error) => {
                        tracing::error!(%error, "safe-cameroon-worker: error processing deliveries");
                        tokio::time::sleep(POLL_INTERVAL).await;
                    }
                }
            }
        }
    }
}
