import type { Comparison, SubscriptionRule } from "../../api/subscriptions";
import type { Translations } from "../../i18n/translations";

const OPERATOR_SYMBOLS: Record<Comparison, string> = {
  GREATER_THAN: ">",
  GREATER_THAN_OR_EQUAL: ">=",
  EQUAL: "==",
  LESS_THAN_OR_EQUAL: "<=",
  LESS_THAN: "<",
};

export function describeRule(rule: SubscriptionRule, t: Translations): string {
  switch (rule.rule) {
    case "INCIDENT_TYPE":
      return t.describeRule.incidentType(rule.values.join(", ") || t.describeRule.noneSelected);
    case "SEVERITY":
      return t.describeRule.severity(OPERATOR_SYMBOLS[rule.operator], rule.value);
    case "EVENT_TYPE":
      return t.describeRule.eventType(rule.values.join(", ") || t.describeRule.noneSelected);
    case "GEOGRAPHY":
      return t.describeRule.geography(rule.area || t.describeRule.none);
  }
}
