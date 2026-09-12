mod cases;
mod error;
mod health;
mod reports;
mod state;

use axum::{
    Router,
    routing::{get, post},
};
use safe_cameroon_infrastructure::postgres::{PostgresCaseRepository, PostgresReportRepository};
use sqlx::postgres::PgPoolOptions;

use crate::state::AppState;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("database connection must succeed");

    let app = Router::new()
        .route("/health", get(health::health))
        .route("/v1/reports", post(reports::create_anonymous_report))
        .route("/v1/cases", post(cases::create_case))
        .route("/v1/cases/{id}", get(cases::get_case))
        .route("/v1/cases/{id}/reports", post(cases::link_report))
        .route("/v1/cases/{id}/events", post(cases::create_case_event))
        .route("/v1/cases/{id}/verify", post(cases::verify_case))
        .route("/v1/cases/{id}/resolve", post(cases::resolve_case))
        .with_state(AppState {
            reports: PostgresReportRepository::new(pool.clone()),
            cases: PostgresCaseRepository::new(pool),
        });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("API listener must bind");
    axum::serve(listener, app)
        .await
        .expect("API server must run");
}
