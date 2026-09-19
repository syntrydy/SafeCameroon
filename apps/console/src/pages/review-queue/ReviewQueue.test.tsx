import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "../../App";
import type { ReportSummary } from "../../api/reports";
import { AuthProvider } from "../../auth/AuthContext";

vi.mock("../../auth/GoogleSignInButton", () => ({
  GoogleSignInButton: ({ onCredential }: { onCredential: (idToken: string) => void }) => (
    <button type="button" onClick={() => onCredential("fake-google-id-token")}>
      Fake Google Sign-In
    </button>
  ),
}));

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

const LOGIN_RESPONSE = {
  token: "a-session-token",
  expires_in_seconds: 3600,
  reviewer_id: "reviewer-1",
  email: "reviewer@example.test",
};

const REPORT: ReportSummary = {
  report_id: "aaaaaaaa-1111-1111-1111-111111111111",
  source_channel: "WEB",
  status: "RECEIVED",
  raw_content: "My child has not returned from school since this afternoon.",
  received_at: "2026-09-15T12:00:00Z",
  reported_incident_type: null,
};

let reports: ReportSummary[];

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string, init?: RequestInit) => {
      const url = new URL(input, "http://localhost");
      const method = init?.method ?? "GET";

      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === "/v1/reports" && method === "GET") {
        const status = url.searchParams.get("status");
        const filtered = status ? reports.filter((report) => report.status === status) : reports;
        return Promise.resolve(jsonResponse(200, filtered));
      }
      if (url.pathname === `/v1/reports/${REPORT.report_id}/attachments` && method === "GET") {
        return Promise.resolve(
          jsonResponse(200, [
            {
              attachment_id: "cccccccc-3333-3333-3333-333333333333",
              object_key: "reports/x/photo.jpg",
              content_type: "image/jpeg",
              size_bytes: 1024,
              checksum: "deadbeef",
            },
          ]),
        );
      }
      if (
        url.pathname === "/v1/attachments/cccccccc-3333-3333-3333-333333333333/download-url" &&
        method === "GET"
      ) {
        return Promise.resolve(
          jsonResponse(200, { download_url: "https://storage.example/signed", expires_in_seconds: 60 }),
        );
      }
      if (url.pathname === `/v1/reports/${REPORT.report_id}/review` && method === "POST") {
        reports = reports.map((report) =>
          report.report_id === REPORT.report_id ? { ...report, status: "UNDER_REVIEW" } : report,
        );
        return Promise.resolve(jsonResponse(200, { ...REPORT, status: "UNDER_REVIEW" }));
      }
      if (url.pathname === "/v1/cases" && method === "POST") {
        const body = JSON.parse(init?.body as string) as { report_id: string; incident_type: string };
        reports = reports.map((report) =>
          report.report_id === body.report_id ? { ...report, status: "LINKED_TO_CASE" } : report,
        );
        return Promise.resolve(
          jsonResponse(201, {
            case_id: "case-1",
            incident_type: body.incident_type,
            status: "REPORTED",
            report_ids: [body.report_id],
            version: 1,
          }),
        );
      }
      if (/\/v1\/cases\/.+\/reports/.test(url.pathname) && method === "POST") {
        const body = JSON.parse(init?.body as string) as { report_id: string };
        reports = reports.map((report) =>
          report.report_id === body.report_id ? { ...report, status: "LINKED_TO_CASE" } : report,
        );
        return Promise.resolve(
          jsonResponse(200, {
            case_id: "existing-case",
            incident_type: "MISSING_CHILD",
            status: "UNDER_REVIEW",
            report_ids: [body.report_id],
            version: 2,
          }),
        );
      }
      throw new Error(`unexpected fetch: ${method} ${url.pathname}`);
    }),
  );
}

async function loginAndReachQueue() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={["/login"]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: "Review queue" });
  return user;
}

beforeEach(() => {
  reports = [{ ...REPORT }];
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ReviewQueue", () => {
  it("lists reports for the default (received) filter", async () => {
    await loginAndReachQueue();

    expect(await screen.findByText(/My child has not returned/)).toBeInTheDocument();
    expect(screen.getByText("RECEIVED")).toBeInTheDocument();
  });

  it("refetches with the selected status filter", async () => {
    const user = await loginAndReachQueue();
    await screen.findByText(/My child has not returned/);

    await user.selectOptions(screen.getByLabelText(/Status:/), "CLOSED");

    await waitFor(() => {
      expect(screen.getByText("No reports match this filter.")).toBeInTheDocument();
    });
  });

  it("creates a case from a report and removes it from the received queue", async () => {
    const user = await loginAndReachQueue();
    const row = (await screen.findByText(/My child has not returned/)).closest("tr")!;

    await user.click(within(row).getByRole("button", { name: "Create case" }));

    await screen.findByRole("status");
    expect(screen.getByRole("status")).toHaveTextContent("Case created.");
    await waitFor(() => {
      expect(screen.getByText("No reports match this filter.")).toBeInTheDocument();
    });
  });

  it("links a report into an existing case", async () => {
    const user = await loginAndReachQueue();
    const row = (await screen.findByText(/My child has not returned/)).closest("tr")!;

    await user.type(within(row).getByPlaceholderText("Existing case id"), "existing-case");
    await user.click(within(row).getByRole("button", { name: "Link to case" }));

    await screen.findByRole("status");
    expect(screen.getByRole("status")).toHaveTextContent("Linked to case.");
  });

  it("pre-fills the incident type from the reporter's own suggestion", async () => {
    reports = [{ ...REPORT, reported_incident_type: "OTHER_PROTECTION_INCIDENT" }];
    const user = await loginAndReachQueue();
    const row = (await screen.findByText(/My child has not returned/)).closest("tr")!;

    expect(within(row).getByText(/Reporter suggested:/)).toHaveTextContent(
      "Reporter suggested: Other protection incident",
    );
    expect(
      within(row).getByLabelText(`Incident type for report ${REPORT.report_id}`),
    ).toHaveValue("OTHER_PROTECTION_INCIDENT");

    await user.click(within(row).getByRole("button", { name: "Create case" }));

    await screen.findByRole("status");
    expect(screen.getByRole("status")).toHaveTextContent("Case created.");
  });

  it("starts reviewing a received report and removes it from the received queue", async () => {
    const user = await loginAndReachQueue();
    const row = (await screen.findByText(/My child has not returned/)).closest("tr")!;

    await user.click(within(row).getByRole("button", { name: "Start review" }));

    await screen.findByRole("status");
    expect(screen.getByRole("status")).toHaveTextContent("Review started.");
    await waitFor(() => {
      expect(screen.getByText("No reports match this filter.")).toBeInTheDocument();
    });
  });

  it("lists a report's attachments and fetches a download link on demand", async () => {
    const user = await loginAndReachQueue();
    const row = (await screen.findByText(/My child has not returned/)).closest("tr")!;

    await user.click(within(row).getByRole("button", { name: "Show attachments" }));
    await within(row).findByText(/image\/jpeg/);

    await user.click(within(row).getByRole("button", { name: "Get download link" }));

    const link = await within(row).findByRole("link", { name: "Open" });
    expect(link).toHaveAttribute("href", "https://storage.example/signed");
  });

  it("surfaces a backend error without losing the row", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((input: string, init?: RequestInit) => {
        const url = new URL(input, "http://localhost");
        const method = init?.method ?? "GET";
        if (url.pathname === "/v1/auth/google") return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
        if (url.pathname === "/v1/reports" && method === "GET") return Promise.resolve(jsonResponse(200, [REPORT]));
        if (url.pathname === "/v1/cases" && method === "POST") {
          return Promise.resolve(
            jsonResponse(409, {
              error: { code: "CASE_ALREADY_EXISTS", message: "A case already exists for this report.", request_id: "req-1" },
            }),
          );
        }
        throw new Error(`unexpected fetch: ${method} ${url.pathname}`);
      }),
    );
    const user = await loginAndReachQueue();
    const row = (await screen.findByText(/My child has not returned/)).closest("tr")!;

    await user.click(within(row).getByRole("button", { name: "Create case" }));

    const alert = await within(row).findByRole("alert");
    expect(alert).toHaveTextContent("A case already exists for this report.");
    expect(alert).toHaveTextContent("req-1");
  });
});
