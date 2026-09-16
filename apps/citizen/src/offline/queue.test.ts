import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { listPendingReports } from "./db";
import { flushPendingReports, submitOrQueue } from "./queue";

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function resetDatabase(): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.deleteDatabase("safecameroon-citizen");
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error as Error);
  });
}

beforeEach(async () => {
  await resetDatabase();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("submitOrQueue", () => {
  it("sends immediately and returns a reference code when the network succeeds", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(201, {
          report_id: "11111111-1111-1111-1111-111111111111",
          reference_code: "SC-abc123",
          status: "RECEIVED",
        }),
      ),
    );

    const outcome = await submitOrQueue("A child is missing.");

    expect(outcome).toEqual({ status: "sent", referenceCode: "SC-abc123" });
    expect(await listPendingReports()).toHaveLength(0);
  });

  it("queues the report when the network is unavailable", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new TypeError("Failed to fetch")),
    );

    const outcome = await submitOrQueue("A child is missing.");

    expect(outcome).toEqual({ status: "queued" });
    const pending = await listPendingReports();
    expect(pending).toHaveLength(1);
    expect(pending[0].content).toBe("A child is missing.");
  });

  it("reports a validation failure immediately without queuing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(400, {
          error: {
            code: "INVALID_REPORT_CONTENT",
            message: "Report content cannot be blank.",
            request_id: "22222222-2222-2222-2222-222222222222",
          },
        }),
      ),
    );

    const outcome = await submitOrQueue("A child is missing.");

    expect(outcome).toEqual({
      status: "rejected",
      message: "Report content cannot be blank.",
    });
    expect(await listPendingReports()).toHaveLength(0);
  });

  it("keeps a report queued when the server keeps failing transiently", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(503, {
          error: { code: "SERVICE_UNAVAILABLE", message: "Try again later.", request_id: null },
        }),
      ),
    );

    const outcome = await submitOrQueue("A child is missing.");

    expect(outcome).toEqual({ status: "queued" });
    expect(await listPendingReports()).toHaveLength(1);
  });
});

describe("flushPendingReports", () => {
  it("sends a previously queued report and reports its reference code", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("offline")));
    await submitOrQueue("A child is missing.");
    expect(await listPendingReports()).toHaveLength(1);

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(201, {
          report_id: "11111111-1111-1111-1111-111111111111",
          reference_code: "SC-def456",
          status: "RECEIVED",
        }),
      ),
    );

    const onSent = vi.fn();
    await flushPendingReports(onSent);

    expect(onSent).toHaveBeenCalledWith("SC-def456");
    expect(await listPendingReports()).toHaveLength(0);
  });

  it("drops a queued report that turns out to be permanently invalid", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("offline")));
    await submitOrQueue("A child is missing.");

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(400, {
          error: { code: "INVALID_REPORT_CONTENT", message: "Too long.", request_id: null },
        }),
      ),
    );

    await flushPendingReports();

    expect(await listPendingReports()).toHaveLength(0);
  });
});
