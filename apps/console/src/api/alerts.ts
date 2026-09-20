import type { IncidentType } from "./cases";
import { apiRequest } from "./client";

// Matches crates/domain/src/alert.rs's enum variants (SCREAMING_SNAKE_CASE on the wire).
export type AlertVisibility = "INTERNAL" | "PARTNER" | "COMMUNITY" | "PUBLIC";
export type AlertStatus = "ACTIVE" | "CANCELLED";
export type Severity = "LOW" | "MEDIUM" | "HIGH" | "CRITICAL";
export type AlertField =
  | "INCIDENT_CATEGORY"
  | "APPROXIMATE_AGE"
  | "LAST_SEEN_GENERAL_AREA"
  | "TIME_WINDOW"
  | "SAFE_DESCRIPTION"
  | "OFFICIAL_CONTACT"
  | "CASE_REFERENCE"
  | "REPORTER_IDENTITY"
  | "INTERNAL_NOTES"
  | "EXACT_LOCATION"
  | "WITNESS_DETAILS";

// The only alert policy this deployment knows about
// (crates/application/src/alert_workflow.rs `resolve_policy`).
export const MISSING_CHILD_COMMUNITY_POLICY_ID = "MISSING_CHILD_COMMUNITY";

// The missing-child community policy's field allowlist, in the order
// crates/domain/src/alert.rs's `missing_child_community_v1` declares them.
export const MISSING_CHILD_COMMUNITY_FIELDS: AlertField[] = [
  "INCIDENT_CATEGORY",
  "APPROXIMATE_AGE",
  "LAST_SEEN_GENERAL_AREA",
  "TIME_WINDOW",
  "SAFE_DESCRIPTION",
  "OFFICIAL_CONTACT",
  "CASE_REFERENCE",
];

export interface AlertFieldValue {
  field: AlertField;
  value: string;
}

// Matches apps/api/src/alerts.rs `AlertResponse`.
export interface Alert {
  alert_id: string;
  case_id: string;
  policy_id: string;
  policy_version: number;
  incident_type: IncidentType;
  severity: Severity;
  visibility: AlertVisibility;
  trigger: string;
  target_geography: string;
  status: AlertStatus;
  fields: AlertFieldValue[];
  version: number;
  can_cancel?: boolean;
}

export interface ListAlertsParams {
  status?: AlertStatus;
  visibility?: AlertVisibility;
  limit?: number;
  offset?: number;
}

export function listAlerts(token: string, params: ListAlertsParams = {}): Promise<Alert[]> {
  const query = new URLSearchParams();
  if (params.status) query.set("status", params.status);
  if (params.visibility) query.set("visibility", params.visibility);
  if (params.limit !== undefined) query.set("limit", String(params.limit));
  if (params.offset !== undefined) query.set("offset", String(params.offset));
  const suffix = query.toString() ? `?${query.toString()}` : "";
  return apiRequest<Alert[]>(`/v1/alerts${suffix}`, { token });
}

export function getAlert(token: string, alertId: string): Promise<Alert> {
  return apiRequest<Alert>(`/v1/alerts/${alertId}`, { token });
}

export interface CreateAlertInput {
  severity: Severity;
  targetGeography: string;
  fields: AlertFieldValue[];
}

// A fresh key per submission, so a duplicate click/retry cannot create two
// alerts from the same case (apps/api/src/idempotency.rs).
export function createAlert(token: string, caseId: string, input: CreateAlertInput): Promise<Alert> {
  return apiRequest<Alert>(`/v1/cases/${caseId}/alerts`, {
    method: "POST",
    token,
    headers: { "Idempotency-Key": crypto.randomUUID() },
    body: {
      policy_id: MISSING_CHILD_COMMUNITY_POLICY_ID,
      severity: input.severity,
      target_geography: input.targetGeography,
      fields: input.fields,
    },
  });
}

export function cancelAlert(token: string, alertId: string): Promise<Alert> {
  return apiRequest<Alert>(`/v1/alerts/${alertId}/cancel`, { method: "POST", token });
}
