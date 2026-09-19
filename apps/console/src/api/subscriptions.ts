import type { CaseEventType, IncidentType } from "./cases";
import { apiRequest } from "./client";
import type { Severity } from "./alerts";

// Matches crates/domain/src/subscription.rs `Comparison`.
export type Comparison = "GREATER_THAN" | "GREATER_THAN_OR_EQUAL" | "EQUAL" | "LESS_THAN_OR_EQUAL" | "LESS_THAN";

// Matches apps/api/src/subscriptions.rs `SubscriptionRuleInput`/`SubscriptionRuleOutput`
// (serde tag "rule", SCREAMING_SNAKE_CASE variant names).
export type SubscriptionRule =
  | { rule: "INCIDENT_TYPE"; values: IncidentType[] }
  | { rule: "SEVERITY"; operator: Comparison; value: Severity }
  | { rule: "EVENT_TYPE"; values: CaseEventType[] }
  | { rule: "GEOGRAPHY"; areas: string[] };

// Matches apps/api/src/subscriptions.rs `SubscriptionResponse`.
export interface Subscription {
  subscription_id: string;
  consumer_id: string;
  version: number;
  rules: SubscriptionRule[];
}

export function createSubscription(
  token: string,
  consumerId: string,
  rules: SubscriptionRule[],
): Promise<Subscription> {
  return apiRequest<Subscription>("/v1/subscriptions", {
    method: "POST",
    token,
    body: { consumer_id: consumerId, rules },
  });
}

export function listSubscriptionsForConsumer(token: string, consumerId: string): Promise<Subscription[]> {
  return apiRequest<Subscription[]>(`/v1/consumers/${consumerId}/subscriptions`, { token });
}

// Every subscription across every consumer, most recently created first --
// apps/api/src/subscriptions.rs `list_all_subscriptions`.
export function listAllSubscriptions(
  token: string,
  params: { limit?: number; offset?: number } = {},
): Promise<Subscription[]> {
  const query = new URLSearchParams();
  if (params.limit !== undefined) query.set("limit", String(params.limit));
  if (params.offset !== undefined) query.set("offset", String(params.offset));
  const suffix = query.toString() ? `?${query.toString()}` : "";
  return apiRequest<Subscription[]>(`/v1/subscriptions${suffix}`, { token });
}

export function updateSubscription(
  token: string,
  subscriptionId: string,
  rules: SubscriptionRule[],
): Promise<Subscription> {
  return apiRequest<Subscription>(`/v1/subscriptions/${subscriptionId}`, {
    method: "PUT",
    token,
    body: { rules },
  });
}

// Matches crates/domain/src/delivery.rs `ChannelType`/`DeliveryStrategy`.
export type ChannelType = "WHATSAPP" | "SMS" | "EMAIL";
export type DeliveryStrategy = "ALL" | "PRIMARY_FALLBACK" | "PRIORITY_LIST";

export interface ChannelEndpoint {
  channel: ChannelType;
  address: string;
}

// Matches apps/api/src/subscriptions.rs `DeliveryPreferenceResponse`.
export interface DeliveryPreference {
  consumer_id: string;
  strategy: DeliveryStrategy;
  channels: ChannelEndpoint[];
}

export function getDeliveryPreference(token: string, consumerId: string): Promise<DeliveryPreference> {
  return apiRequest<DeliveryPreference>(`/v1/consumers/${consumerId}/delivery-preference`, { token });
}

export function setDeliveryPreference(
  token: string,
  consumerId: string,
  strategy: DeliveryStrategy,
  channels: ChannelEndpoint[],
): Promise<DeliveryPreference> {
  return apiRequest<DeliveryPreference>(`/v1/consumers/${consumerId}/delivery-preference`, {
    method: "PUT",
    token,
    body: { strategy, channels },
  });
}
