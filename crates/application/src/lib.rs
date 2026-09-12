//! Application use cases. Infrastructure implements ports introduced here.

use safe_cameroon_domain::{Case, CaseEvent, CaseStatus, TransitionError};

/// A state transition is intentionally a use case, not an incidental data update.
pub fn transition_case(case_: &mut Case, next: CaseStatus) -> Result<CaseEvent, TransitionError> {
    case_.transition_to(next)
}
