use safe_cameroon_infrastructure::postgres::{PostgresCaseRepository, PostgresReportRepository};

#[derive(Clone)]
pub struct AppState {
    pub reports: PostgresReportRepository,
    pub cases: PostgresCaseRepository,
}
