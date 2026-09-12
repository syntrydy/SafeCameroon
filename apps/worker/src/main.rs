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

    println!("safe-cameroon-worker: polling for deliveries every {POLL_INTERVAL:?}");
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("safe-cameroon-worker: shutting down");
                break;
            }
            result = dispatch::process_batch(&deliveries, &alerts, &registry, BATCH_SIZE) => {
                match result {
                    Ok(0) => tokio::time::sleep(POLL_INTERVAL).await,
                    Ok(count) => println!("safe-cameroon-worker: processed {count} delivery job(s)"),
                    Err(error) => {
                        eprintln!("safe-cameroon-worker: error processing deliveries: {error}");
                        tokio::time::sleep(POLL_INTERVAL).await;
                    }
                }
            }
        }
    }
}
