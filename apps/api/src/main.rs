use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use safe_cameroon_application::{ReportValidationError, prepare_anonymous_report};
use safe_cameroon_infrastructure::postgres::PostgresReportRepository;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    reports: PostgresReportRepository,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "safe-cameroon-api",
    })
}

#[derive(Deserialize)]
struct CreateReportRequest {
    content: String,
}

#[derive(Serialize)]
struct CreateReportResponse {
    report_id: Uuid,
    reference_code: String,
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
    request_id: Uuid,
}

struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    request_id: Uuid,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: self.message,
                    request_id: self.request_id,
                },
            }),
        )
            .into_response()
    }
}

async fn create_anonymous_report(
    State(state): State<AppState>,
    Json(request): Json<CreateReportRequest>,
) -> Result<(StatusCode, Json<CreateReportResponse>), ApiError> {
    let request_id = Uuid::new_v4();
    let submission = prepare_anonymous_report(request.content, request_id).map_err(|error| {
        let (code, message) = match error {
            ReportValidationError::EmptyContent => {
                ("INVALID_REPORT_CONTENT", "Report content cannot be blank.")
            }
            ReportValidationError::ContentTooLong => {
                ("INVALID_REPORT_CONTENT", "Report content is too long.")
            }
        };
        ApiError {
            status: StatusCode::BAD_REQUEST,
            code,
            message,
            request_id,
        }
    })?;

    state
        .reports
        .submit_anonymous(&submission)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "REPORT_PERSISTENCE_FAILED",
            message: "The report could not be saved. Please try again.",
            request_id,
        })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateReportResponse {
            report_id: submission.report.id.as_uuid(),
            reference_code: submission.reference_code,
            status: "RECEIVED",
        }),
    ))
}

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("database connection must succeed");
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/reports", post(create_anonymous_report))
        .with_state(AppState {
            reports: PostgresReportRepository::new(pool),
        });
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("API listener must bind");
    axum::serve(listener, app)
        .await
        .expect("API server must run");
}
