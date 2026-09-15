import { apiRequest } from "./client";

// Matches crates/domain/src/case.rs's enum variants (SCREAMING_SNAKE_CASE on the wire).
export type IncidentType = "MISSING_CHILD" | "OTHER_PROTECTION_INCIDENT";
export type CaseStatus =
  | "REPORTED"
  | "UNDER_REVIEW"
  | "VERIFIED"
  | "ACTIVE"
  | "RESOLVED"
  | "CANCELLED"
  | "REJECTED";

// Matches apps/api/src/cases.rs `CaseResponse`.
export interface Case {
  case_id: string;
  incident_type: IncidentType;
  status: CaseStatus;
  report_ids: string[];
  version: number;
}

export function createCase(token: string, reportId: string, incidentType: IncidentType): Promise<Case> {
  return apiRequest<Case>("/v1/cases", {
    method: "POST",
    token,
    body: { report_id: reportId, incident_type: incidentType },
  });
}

export function linkReportToCase(token: string, caseId: string, reportId: string): Promise<Case> {
  return apiRequest<Case>(`/v1/cases/${caseId}/reports`, {
    method: "POST",
    token,
    body: { report_id: reportId },
  });
}
