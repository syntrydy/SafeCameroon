/// Worker entry point. Outbox polling is deliberately not implemented until the
/// database transaction, idempotency, and retry contracts are in place.
#[tokio::main]
async fn main() {
    println!("safe-cameroon-worker: no jobs configured");
}
