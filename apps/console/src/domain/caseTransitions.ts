import type { CaseStatus } from "../api/cases";

// Mirrors crates/domain/src/case.rs's `is_allowed_transition` exactly, so
// the console only ever offers an action the backend will actually accept.
// Keep these two in sync if the domain's transition table changes.
const ALLOWED_TRANSITIONS: Record<CaseStatus, CaseStatus[]> = {
  REPORTED: ["UNDER_REVIEW"],
  UNDER_REVIEW: ["VERIFIED", "REJECTED"],
  VERIFIED: ["ACTIVE", "CANCELLED"],
  ACTIVE: ["RESOLVED", "CANCELLED"],
  RESOLVED: [],
  CANCELLED: [],
  REJECTED: [],
};

export function nextStatusOptions(status: CaseStatus): CaseStatus[] {
  return ALLOWED_TRANSITIONS[status];
}
