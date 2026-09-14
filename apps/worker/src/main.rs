mod dispatch;
mod subscription_matching;

use std::sync::Arc;
use std::time::Duration;

use safe_cameroon_application::channel::ChannelRegistry;
use safe_cameroon_infrastructure::channels::{EmailChannel, SmsChannel, WhatsAppChannel};
use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresDeliveryPreferenceRepository, PostgresDeliveryRepository,
    PostgresOutboxRepository, PostgresSubscriptionRepository,
};
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
    let alerts = PostgresAlertRepository::new(pool.clone());
    let outbox = PostgresOutboxRepository::new(pool.clone());
    let subscriptions = PostgresSubscriptionRepository::new(pool.clone());
    let delivery_preferences = PostgresDeliveryPreferenceRepository::new(pool);

    // Mock/sandbox adapters: real vendor credentials are not available yet
    // (prompt 08). Endpoint validation and provider-error mapping are real;
    // only the actual network call is simulated.
    let mut registry = ChannelRegistry::new();
    registry.register(Arc::new(WhatsAppChannel));
    registry.register(Arc::new(SmsChannel));
    registry.register(Arc::new(EmailChannel));

    tracing::info!(poll_interval = ?POLL_INTERVAL, "safe-cameroon-worker: polling for alerts and deliveries");
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("safe-cameroon-worker: shutting down");
                break;
            }
            result = process_cycle(&outbox, &alerts, &subscriptions, &delivery_preferences, &deliveries, &registry) => {
                match result {
                    Ok(0) => tokio::time::sleep(POLL_INTERVAL).await,
                    Ok(count) => tracing::info!(count, "safe-cameroon-worker: processed job(s)"),
                    Err(error) => {
                        tracing::error!(%error, "safe-cameroon-worker: error processing jobs");
                        tokio::time::sleep(POLL_INTERVAL).await;
                    }
                }
            }
        }
    }
}

/// One full cycle: first turn any newly created alerts into planned
/// deliveries (`subscription_matching`), then dispatch whatever is ready to
/// send (`dispatch`) — including deliveries this same cycle just planned.
/// Both steps are independently batch-bounded and idempotent, so running
/// them back to back in one cycle is just an ordering choice, not a
/// correctness requirement.
async fn process_cycle(
    outbox: &PostgresOutboxRepository,
    alerts: &PostgresAlertRepository,
    subscriptions: &PostgresSubscriptionRepository,
    delivery_preferences: &PostgresDeliveryPreferenceRepository,
    deliveries: &PostgresDeliveryRepository,
    registry: &ChannelRegistry,
) -> Result<usize, sqlx::Error> {
    let matched = subscription_matching::process_batch(
        outbox,
        alerts,
        subscriptions,
        delivery_preferences,
        deliveries,
        BATCH_SIZE,
    )
    .await?;
    let dispatched = dispatch::process_batch(deliveries, alerts, registry, BATCH_SIZE).await?;
    Ok(matched + dispatched)
}
