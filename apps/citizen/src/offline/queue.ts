import { isPermanentFailure } from "../api/client";
import { submitReport, uploadPhoto, type IncidentType, type PhotoContentType } from "../api/reports";
import { deletePendingReport, listPendingReports, putPendingReport, type PendingReport } from "./db";
import { addReceipt } from "./receipts";

export interface NewReportPhoto {
  blob: Blob;
  contentType: PhotoContentType;
}

export type SubmitOutcome =
  | { status: "sent"; referenceCode: string }
  | { status: "queued" }
  | { status: "rejected"; message: string };

function generateId(): string {
  return crypto.randomUUID();
}

async function toPendingPhoto(
  photo: NewReportPhoto | undefined,
): Promise<PendingReport["photo"]> {
  if (!photo) {
    return undefined;
  }
  return { data: await photo.blob.arrayBuffer(), contentType: photo.contentType };
}

async function send(report: PendingReport): Promise<string> {
  const result = await submitReport(report.content, report.id, report.incidentType);
  if (report.photo) {
    try {
      const blob = new Blob([report.photo.data], { type: report.photo.contentType });
      await uploadPhoto(result.report_id, blob, report.photo.contentType);
    } catch {
      // The report text is the safety-critical part and already succeeded;
      // a failed photo attach is best-effort, not worth re-queuing the
      // whole report for (prompts/03_REPORTING.md: "uploads photo if
      // available").
    }
  }
  return result.reference_code;
}

/** Tries to submit immediately; only falls back to the offline queue when
 * the attempt fails for a transient reason (no network, 5xx, rate limit). A
 * genuine validation failure is reported back instead of queued, since
 * retrying identical content can never succeed. */
export async function submitOrQueue(
  content: string,
  incidentType?: IncidentType,
  photo?: NewReportPhoto,
): Promise<SubmitOutcome> {
  const pending: PendingReport = {
    id: generateId(),
    content,
    incidentType,
    photo: await toPendingPhoto(photo),
    createdAt: Date.now(),
    attempts: 0,
  };

  try {
    const referenceCode = await send(pending);
    addReceipt(referenceCode);
    return { status: "sent", referenceCode };
  } catch (error) {
    if (isPermanentFailure(error)) {
      return {
        status: "rejected",
        message: error instanceof Error ? error.message : "This report could not be sent.",
      };
    }
    await putPendingReport({ ...pending, attempts: 1 });
    return { status: "queued" };
  }
}

/** Retries every queued report once. Safe to call opportunistically (on
 * reconnect, on an interval, on tab focus) -- a report already delivered
 * keeps the same Idempotency-Key, so a lost response never double-submits. */
export async function flushPendingReports(
  onSent?: (referenceCode: string) => void,
): Promise<void> {
  const pending = await listPendingReports();
  for (const report of pending) {
    try {
      const referenceCode = await send(report);
      await deletePendingReport(report.id);
      addReceipt(referenceCode);
      onSent?.(referenceCode);
    } catch (error) {
      if (isPermanentFailure(error)) {
        await deletePendingReport(report.id);
        continue;
      }
      await putPendingReport({ ...report, attempts: report.attempts + 1 });
    }
  }
}

export async function countPendingReports(): Promise<number> {
  return (await listPendingReports()).length;
}

const SYNC_INTERVAL_MS = 20_000;

/** Starts opportunistic background retries; returns a function that stops
 * them. Safe to call once per app lifetime -- there is normally nothing to
 * unmount in this single-page app, but tests need to be able to stop it. */
export function startBackgroundSync(onSent: (referenceCode: string) => void): () => void {
  const flush = () => void flushPendingReports(onSent);
  const onVisible = () => {
    if (document.visibilityState === "visible") {
      flush();
    }
  };

  const intervalId = window.setInterval(flush, SYNC_INTERVAL_MS);
  window.addEventListener("online", flush);
  document.addEventListener("visibilitychange", onVisible);
  flush();

  return () => {
    window.clearInterval(intervalId);
    window.removeEventListener("online", flush);
    document.removeEventListener("visibilitychange", onVisible);
  };
}
