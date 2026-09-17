import { apiRequest } from "./client";

export type IncidentType = "MISSING_CHILD" | "OTHER_PROTECTION_INCIDENT";
export type Severity = "LOW" | "MEDIUM" | "HIGH" | "CRITICAL";

export interface AlertSubscriptionRules {
  incidentTypes: IncidentType[];
  minimumSeverity: Severity;
  geography: string[];
}

interface CreateCitizenSubscriptionResult {
  consumer_id: string;
  subscription_id: string;
  management_token: string;
}

function toRequestBody(rules: AlertSubscriptionRules, pushSubscription?: PushSubscriptionJSON) {
  return {
    incident_types: rules.incidentTypes,
    minimum_severity: rules.minimumSeverity,
    geography: rules.geography,
    ...(pushSubscription ? { push_subscription: pushSubscription } : {}),
  };
}

export function createCitizenSubscription(
  rules: AlertSubscriptionRules,
  pushSubscription: PushSubscriptionJSON,
): Promise<CreateCitizenSubscriptionResult> {
  return apiRequest<CreateCitizenSubscriptionResult>("/v1/citizen-subscriptions", {
    method: "POST",
    body: JSON.stringify(toRequestBody(rules, pushSubscription)),
  });
}

export function getCitizenSubscription(
  subscriptionId: string,
  managementToken: string,
): Promise<GetCitizenSubscriptionResult> {
  return apiRequest<GetCitizenSubscriptionResult>(`/v1/citizen-subscriptions/${subscriptionId}`, {
    method: "GET",
    headers: { "Management-Token": managementToken },
  });
}

interface GetCitizenSubscriptionResult {
  incident_types: IncidentType[];
  minimum_severity: Severity;
  geography: string[];
}

export function updateCitizenSubscription(
  subscriptionId: string,
  managementToken: string,
  rules: AlertSubscriptionRules,
  pushSubscription?: PushSubscriptionJSON,
): Promise<GetCitizenSubscriptionResult> {
  return apiRequest<GetCitizenSubscriptionResult>(`/v1/citizen-subscriptions/${subscriptionId}`, {
    method: "PUT",
    headers: { "Management-Token": managementToken },
    body: JSON.stringify(toRequestBody(rules, pushSubscription)),
  });
}

export function cancelCitizenSubscription(
  subscriptionId: string,
  managementToken: string,
): Promise<void> {
  return apiRequest<void>(`/v1/citizen-subscriptions/${subscriptionId}/cancel`, {
    method: "POST",
    headers: { "Management-Token": managementToken },
  });
}
