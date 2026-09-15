import { describe, expect, it } from "vitest";

import { canCreateAlert } from "./alertEligibility";

describe("canCreateAlert", () => {
  it("allows verified-or-later missing-child cases", () => {
    expect(canCreateAlert("VERIFIED", "MISSING_CHILD")).toBe(true);
    expect(canCreateAlert("ACTIVE", "MISSING_CHILD")).toBe(true);
    expect(canCreateAlert("RESOLVED", "MISSING_CHILD")).toBe(true);
  });

  it("rejects cases not yet verified", () => {
    expect(canCreateAlert("REPORTED", "MISSING_CHILD")).toBe(false);
    expect(canCreateAlert("UNDER_REVIEW", "MISSING_CHILD")).toBe(false);
  });

  it("rejects terminal statuses the policy never reaches from", () => {
    expect(canCreateAlert("CANCELLED", "MISSING_CHILD")).toBe(false);
    expect(canCreateAlert("REJECTED", "MISSING_CHILD")).toBe(false);
  });

  it("rejects an incident type the only known policy does not cover", () => {
    expect(canCreateAlert("VERIFIED", "OTHER_PROTECTION_INCIDENT")).toBe(false);
  });
});
