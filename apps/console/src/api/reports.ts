import { apiRequest } from "./client";

// Matches apps/api/src/reports.rs `ReportStatus`/`ReportSourceChannel` and
// crates/domain/src/report.rs's enum variants (SCREAMING_SNAKE_CASE on the wire).
export type ReportStatus = "RECEIVED" | "UNDER_REVIEW" | "LINKED_TO_CASE" | "CLOSED";
export type ReportSourceChannel = "WEB" | "SMS" | "WHATSAPP" | "PHONE" | "PARTNER_API";

// Matches apps/api/src/reports.rs `ReportSummaryResponse`. There is no
// single-report GET — this list is a reviewer's only way to read report
// content before deciding whether to open or link a case.
export interface ReportSummary {
  report_id: string;
  source_channel: ReportSourceChannel;
  status: ReportStatus;
  raw_content: string;
  received_at: string;
  // The reporter's own guess, if the intake UI asked. Never authoritative --
  // a reviewer still explicitly chooses the incident type below.
  reported_incident_type: "MISSING_CHILD" | "OTHER_PROTECTION_INCIDENT" | null;
}

export interface ListReportsParams {
  status?: ReportStatus;
  limit?: number;
  offset?: number;
}

export function listReports(token: string, params: ListReportsParams = {}): Promise<ReportSummary[]> {
  const query = new URLSearchParams();
  if (params.status) query.set("status", params.status);
  if (params.limit !== undefined) query.set("limit", String(params.limit));
  if (params.offset !== undefined) query.set("offset", String(params.offset));
  const suffix = query.toString() ? `?${query.toString()}` : "";
  return apiRequest<ReportSummary[]>(`/v1/reports${suffix}`, { token });
}

// Moves a report from RECEIVED to UNDER_REVIEW (apps/api/src/reports.rs
// `start_report_review`) -- a queue-triage signal only, not required
// before creating or linking a case.
export function startReportReview(token: string, reportId: string): Promise<ReportSummary> {
  return apiRequest<ReportSummary>(`/v1/reports/${reportId}/review`, { method: "POST", token });
}
