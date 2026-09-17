import { describe, expect, it } from "vitest";

import { describeRule } from "./describeRule";
import { translations } from "../../i18n/translations";

const t = translations.en;

describe("describeRule", () => {
  it("describes an incident type rule", () => {
    expect(describeRule({ rule: "INCIDENT_TYPE", values: ["MISSING_CHILD"] }, t)).toBe(
      "Incident type: MISSING_CHILD",
    );
  });

  it("describes a severity rule with its comparison symbol", () => {
    expect(describeRule({ rule: "SEVERITY", operator: "GREATER_THAN_OR_EQUAL", value: "HIGH" }, t)).toBe(
      "Severity >= HIGH",
    );
  });

  it("describes an event type rule", () => {
    expect(describeRule({ rule: "EVENT_TYPE", values: ["CASE_VERIFIED", "CASE_ACTIVATED"] }, t)).toBe(
      "Event type: CASE_VERIFIED, CASE_ACTIVATED",
    );
  });

  it("describes a geography rule", () => {
    expect(describeRule({ rule: "GEOGRAPHY", areas: ["Douala"] }, t)).toBe("Geography: Douala");
  });

  it("describes a geography rule with multiple areas", () => {
    expect(describeRule({ rule: "GEOGRAPHY", areas: ["Douala", "Yaounde"] }, t)).toBe(
      "Geography: Douala, Yaounde",
    );
  });

  it("falls back to a placeholder for an empty multi-value rule", () => {
    expect(describeRule({ rule: "INCIDENT_TYPE", values: [] }, t)).toBe("Incident type: (none selected)");
  });
});
