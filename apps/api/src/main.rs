use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use safe_cameroon_application::{ReportValidationError, prepare_anonymous_report};
use safe_cameroon_infrastructure::postgres::PostgresReportRepository;
use safe_cameroon_infrastructure::postgres::SubmissionResult;
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
    headers: axum::http::HeaderMap,
    Json(request): Json<CreateReportRequest>,
) -> Result<(StatusCode, Json<CreateReportResponse>), ApiError> {
    let request_id = Uuid::new_v4();
    let idempotency_key = headers
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(key) = idempotency_key {
        if !(8..=256).contains(&key.len()) {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                code: "INVALID_IDEMPOTENCY_KEY",
                message: "Idempotency-Key must be between 8 and 256 characters.",
                request_id,
            });
        }
    }
    let submission = prepare_anonymous_report(request.content, request_id, idempotency_key)
        .map_err(|error| {
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

    let result = state
        .reports
        .submit_anonymous(&submission)
        .await
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "REPORT_PERSISTENCE_FAILED",
            message: "The report could not be saved. Please try again.",
            request_id,
        })?;

    if matches!(result, SubmissionResult::Duplicate { .. }) {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            code: "IDEMPOTENCY_KEY_REUSED",
            message: "This Idempotency-Key was already used for a report.",
            request_id,
        });
    }

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
