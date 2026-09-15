import { describe, expect, it } from "vitest";

import { nextStatusOptions } from "./caseTransitions";

describe("nextStatusOptions", () => {
  it("matches the domain's transition table", () => {
    expect(nextStatusOptions("REPORTED")).toEqual(["UNDER_REVIEW"]);
    expect(nextStatusOptions("UNDER_REVIEW")).toEqual(["VERIFIED", "REJECTED"]);
    expect(nextStatusOptions("VERIFIED")).toEqual(["ACTIVE", "CANCELLED"]);
    expect(nextStatusOptions("ACTIVE")).toEqual(["RESOLVED", "CANCELLED"]);
  });

  it("offers no further transitions from a terminal status", () => {
    expect(nextStatusOptions("RESOLVED")).toEqual([]);
    expect(nextStatusOptions("CANCELLED")).toEqual([]);
    expect(nextStatusOptions("REJECTED")).toEqual([]);
  });
});
