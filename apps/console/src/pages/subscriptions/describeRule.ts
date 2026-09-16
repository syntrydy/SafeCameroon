import type { Comparison, SubscriptionRule } from "../../api/subscriptions";

const OPERATOR_SYMBOLS: Record<Comparison, string> = {
  GREATER_THAN: ">",
  GREATER_THAN_OR_EQUAL: ">=",
  EQUAL: "==",
  LESS_THAN_OR_EQUAL: "<=",
  LESS_THAN: "<",
};

export function describeRule(rule: SubscriptionRule): string {
  switch (rule.rule) {
    case "INCIDENT_TYPE":
      return `Incident type: ${rule.values.join(", ") || "(none selected)"}`;
    case "SEVERITY":
      return `Severity ${OPERATOR_SYMBOLS[rule.operator]} ${rule.value}`;
    case "EVENT_TYPE":
      return `Event type: ${rule.values.join(", ") || "(none selected)"}`;
    case "GEOGRAPHY":
      return `Geography: ${rule.area || "(none)"}`;
  }
}
