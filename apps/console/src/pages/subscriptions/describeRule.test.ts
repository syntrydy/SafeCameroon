import { describe, expect, it } from "vitest";

import { describeRule } from "./describeRule";

describe("describeRule", () => {
  it("describes an incident type rule", () => {
    expect(describeRule({ rule: "INCIDENT_TYPE", values: ["MISSING_CHILD"] })).toBe(
      "Incident type: MISSING_CHILD",
    );
  });

  it("describes a severity rule with its comparison symbol", () => {
    expect(describeRule({ rule: "SEVERITY", operator: "GREATER_THAN_OR_EQUAL", value: "HIGH" })).toBe(
      "Severity >= HIGH",
    );
  });

  it("describes an event type rule", () => {
    expect(describeRule({ rule: "EVENT_TYPE", values: ["CASE_VERIFIED", "CASE_ACTIVATED"] })).toBe(
      "Event type: CASE_VERIFIED, CASE_ACTIVATED",
    );
  });

  it("describes a geography rule", () => {
    expect(describeRule({ rule: "GEOGRAPHY", area: "Douala" })).toBe("Geography: Douala");
  });

  it("falls back to a placeholder for an empty multi-value rule", () => {
    expect(describeRule({ rule: "INCIDENT_TYPE", values: [] })).toBe("Incident type: (none selected)");
  });
});
