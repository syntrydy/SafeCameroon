import { apiRequest } from "./client";

// Matches crates/domain/src/consumer.rs `ConsumerType`.
export type ConsumerType = "ORGANIZATION" | "CITIZEN";

// Matches apps/api/src/consumers.rs `ConsumerResponse`.
export interface Consumer {
  consumer_id: string;
  name: string;
  consumer_type: ConsumerType;
}

export function registerConsumer(token: string, name: string, consumerType: ConsumerType): Promise<Consumer> {
  return apiRequest<Consumer>("/v1/consumers", {
    method: "POST",
    token,
    body: { name, consumer_type: consumerType },
  });
}

export function getConsumer(token: string, consumerId: string): Promise<Consumer> {
  return apiRequest<Consumer>(`/v1/consumers/${consumerId}`, { token });
}
