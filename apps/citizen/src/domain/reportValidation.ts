// Mirrors crates/application/src/lib.rs::prepare_anonymous_report so the
// form can reject an invalid report instantly, without a round trip --
// important on a slow connection where every request has a real cost.
export const MAX_REPORT_CONTENT_CHARS = 10_000;

export type ReportContentError = "EMPTY" | "TOO_LONG";

export function validateReportContent(rawContent: string): ReportContentError | null {
  const normalized = rawContent.trim();
  if (normalized.length === 0) {
    return "EMPTY";
  }
  if ([...normalized].length > MAX_REPORT_CONTENT_CHARS) {
    return "TOO_LONG";
  }
  return null;
}
