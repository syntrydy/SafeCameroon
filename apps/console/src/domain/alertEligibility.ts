import type { CaseStatus, IncidentType } from "../api/cases";

// Mirrors crates/domain/src/alert.rs's `Alert::create_from_case` checks
// (is_verified_or_later + incident-type match against the only policy this
// deployment knows, MISSING_CHILD_COMMUNITY) so the console only offers
// "create alert" when the backend would actually accept it.
export function canCreateAlert(status: CaseStatus, incidentType: IncidentType): boolean {
  const caseIsVerifiedOrLater = status === "VERIFIED" || status === "ACTIVE" || status === "RESOLVED";
  return caseIsVerifiedOrLater && incidentType === "MISSING_CHILD";
}
