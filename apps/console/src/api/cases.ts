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

export type CaseEventType =
  | "CASE_CREATED"
  | "CASE_REPORT_LINKED"
  | "CASE_UNDER_REVIEW"
  | "CASE_VERIFIED"
  | "CASE_ACTIVATED"
  | "CASE_RESOLVED"
  | "CASE_CANCELLED"
  | "CASE_REJECTED";

// Matches apps/api/src/cases.rs `CaseResponse`.
export interface Case {
  case_id: string;
  incident_type: IncidentType;
  status: CaseStatus;
  report_ids: string[];
  version: number;
}

// Matches apps/api/src/cases.rs `CaseEventHistoryResponse`.
export interface CaseEvent {
  id: string;
  case_id: string;
  event_type: CaseEventType;
  aggregate_version: number;
  actor_type: string;
  actor_id: string | null;
  occurred_at: string;
}

export interface ListCasesParams {
  status?: CaseStatus;
  incidentType?: IncidentType;
  limit?: number;
  offset?: number;
}

export function listCases(token: string, params: ListCasesParams = {}): Promise<Case[]> {
  const query = new URLSearchParams();
  if (params.status) query.set("status", params.status);
  if (params.incidentType) query.set("incident_type", params.incidentType);
  if (params.limit !== undefined) query.set("limit", String(params.limit));
  if (params.offset !== undefined) query.set("offset", String(params.offset));
  const suffix = query.toString() ? `?${query.toString()}` : "";
  return apiRequest<Case[]>(`/v1/cases${suffix}`, { token });
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

export function getCase(token: string, caseId: string): Promise<Case> {
  return apiRequest<Case>(`/v1/cases/${caseId}`, { token });
}

export function listCaseEvents(token: string, caseId: string): Promise<CaseEvent[]> {
  return apiRequest<CaseEvent[]>(`/v1/cases/${caseId}/events`, { token });
}

export function verifyCase(token: string, caseId: string): Promise<Case> {
  return apiRequest<Case>(`/v1/cases/${caseId}/verify`, { method: "POST", token });
}

export function resolveCase(token: string, caseId: string): Promise<Case> {
  return apiRequest<Case>(`/v1/cases/${caseId}/resolve`, { method: "POST", token });
}

// Every transition without a dedicated shorthand endpoint (UNDER_REVIEW,
// ACTIVE, CANCELLED, REJECTED) — verify/resolve above cover the other two.
export function createCaseEvent(token: string, caseId: string, to: CaseStatus): Promise<Case> {
  return apiRequest<Case>(`/v1/cases/${caseId}/events`, {
    method: "POST",
    token,
    body: { to },
  });
}
