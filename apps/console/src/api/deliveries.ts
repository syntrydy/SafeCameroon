import { apiRequest } from "./client";
import type { ChannelType } from "./subscriptions";

// Matches crates/domain/src/delivery.rs `DeliveryStatus`.
export type DeliveryStatus = "QUEUED" | "SENDING" | "SENT" | "DELIVERED" | "RETRYING" | "FAILED_PERMANENTLY";

// Matches apps/api/src/deliveries.rs `DeliveryResponse`.
export interface Delivery {
  delivery_id: string;
  alert_id: string;
  consumer_id: string;
  channel: ChannelType;
  endpoint_address: string;
  tier: number;
  status: DeliveryStatus;
  attempt_count: number;
  max_attempts: number;
  version: number;
}

export type DeliveryAttemptOutcome = "SENT" | "FAILED";

// Matches apps/api/src/deliveries.rs `DeliveryAttemptResponse`.
export interface DeliveryAttempt {
  attempt_number: number;
  outcome: DeliveryAttemptOutcome;
  provider_message_id: string | null;
  retryable: boolean | null;
  failure_reason: string | null;
}

// Matches apps/api/src/deliveries.rs `DeliveryDetailResponse` (the delivery
// fields are flattened alongside `attempts` on the wire).
export type DeliveryDetail = Delivery & { attempts: DeliveryAttempt[] };

export function listDeliveriesForAlert(token: string, alertId: string): Promise<Delivery[]> {
  return apiRequest<Delivery[]>(`/v1/alerts/${alertId}/deliveries`, { token });
}

export function getDelivery(token: string, deliveryId: string): Promise<DeliveryDetail> {
  return apiRequest<DeliveryDetail>(`/v1/deliveries/${deliveryId}`, { token });
}
