import { describe, expect, it } from "vitest";

import { MAX_REPORT_CONTENT_CHARS, validateReportContent } from "./reportValidation";

describe("validateReportContent", () => {
  it("accepts non-blank content within the length limit", () => {
    expect(validateReportContent("A child is missing.")).toBeNull();
  });

  it("rejects blank or whitespace-only content", () => {
    expect(validateReportContent("   \n  ")).toBe("EMPTY");
  });

  it("rejects content over the character limit", () => {
    expect(validateReportContent("a".repeat(MAX_REPORT_CONTENT_CHARS + 1))).toBe("TOO_LONG");
  });

  it("accepts content exactly at the character limit", () => {
    expect(validateReportContent("a".repeat(MAX_REPORT_CONTENT_CHARS))).toBeNull();
  });
});
