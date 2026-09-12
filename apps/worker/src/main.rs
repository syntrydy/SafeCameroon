mod dispatch;

use std::sync::Arc;
use std::time::Duration;

use safe_cameroon_application::channel::ChannelRegistry;
use safe_cameroon_domain::ChannelType;
use safe_cameroon_infrastructure::channels::LoggingChannel;
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

    // LoggingChannel is a stand-in for every channel until prompt 08 adds
    // real mock/sandbox WhatsApp/SMS/Email adapters behind this same port.
    let mut registry = ChannelRegistry::new();
    registry.register(Arc::new(LoggingChannel::new(ChannelType::WhatsApp)));
    registry.register(Arc::new(LoggingChannel::new(ChannelType::Sms)));
    registry.register(Arc::new(LoggingChannel::new(ChannelType::Email)));

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
