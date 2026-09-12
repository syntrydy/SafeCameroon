use safe_cameroon_infrastructure::postgres::{
    PostgresAlertRepository, PostgresCaseRepository, PostgresReportRepository,
};

#[derive(Clone)]
pub struct AppState {
    pub reports: PostgresReportRepository,
    pub cases: PostgresCaseRepository,
    pub alerts: PostgresAlertRepository,
}
