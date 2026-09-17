import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "../../App";
import type { Case, CaseEvent } from "../../api/cases";
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

const CASE_ID = "aaaaaaaa-1111-1111-1111-111111111111";
const REPORT_ID = "bbbbbbbb-2222-2222-2222-222222222222";
const ALERT_ID = "dddddddd-4444-4444-4444-444444444444";

let caseData: Case;
let events: CaseEvent[];

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string, init?: RequestInit) => {
      const url = new URL(input, "http://localhost");
      const method = init?.method ?? "GET";

      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === `/v1/cases/${CASE_ID}` && method === "GET") {
        return Promise.resolve(jsonResponse(200, caseData));
      }
      if (url.pathname === `/v1/cases/${CASE_ID}/events` && method === "GET") {
        return Promise.resolve(jsonResponse(200, events));
      }
      if (url.pathname === `/v1/cases/${CASE_ID}/verify` && method === "POST") {
        caseData = { ...caseData, status: "VERIFIED", version: caseData.version + 1 };
        return Promise.resolve(jsonResponse(200, caseData));
      }
      if (url.pathname === `/v1/cases/${CASE_ID}/events` && method === "POST") {
        const body = JSON.parse(init?.body as string) as { to: Case["status"] };
        caseData = { ...caseData, status: body.to, version: caseData.version + 1 };
        return Promise.resolve(jsonResponse(200, caseData));
      }
      if (url.pathname === `/v1/reports/${REPORT_ID}/attachments` && method === "GET") {
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
      if (url.pathname === `/v1/cases/${CASE_ID}/alerts` && method === "POST") {
        const body = JSON.parse(init?.body as string) as {
          severity: string;
          target_geography: string;
          fields: { field: string; value: string }[];
        };
        return Promise.resolve(
          jsonResponse(201, {
            alert_id: ALERT_ID,
            case_id: CASE_ID,
            policy_id: "MISSING_CHILD_COMMUNITY",
            policy_version: 1,
            incident_type: "MISSING_CHILD",
            severity: body.severity,
            visibility: "COMMUNITY",
            trigger: "CASE_VERIFIED",
            target_geography: body.target_geography,
            status: "ACTIVE",
            fields: body.fields,
            version: 1,
          }),
        );
      }
      if (url.pathname === `/v1/alerts/${ALERT_ID}` && method === "GET") {
        return Promise.resolve(
          jsonResponse(200, {
            alert_id: ALERT_ID,
            case_id: CASE_ID,
            policy_id: "MISSING_CHILD_COMMUNITY",
            policy_version: 1,
            incident_type: "MISSING_CHILD",
            severity: "HIGH",
            visibility: "COMMUNITY",
            trigger: "CASE_VERIFIED",
            target_geography: "Douala, Bonamoussadi",
            status: "ACTIVE",
            fields: [],
            version: 1,
          }),
        );
      }
      throw new Error(`unexpected fetch: ${method} ${url.pathname}`);
    }),
  );
}

async function loginAndReachCase() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={[`/cases/${CASE_ID}`]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: `Case ${CASE_ID.slice(0, 8)}` });
  return user;
}

beforeEach(() => {
  caseData = {
    case_id: CASE_ID,
    incident_type: "MISSING_CHILD",
    status: "UNDER_REVIEW",
    report_ids: [REPORT_ID],
    version: 2,
  };
  events = [
    {
      id: "event-1",
      case_id: CASE_ID,
      event_type: "CASE_CREATED",
      aggregate_version: 1,
      actor_type: "REVIEWER",
      actor_id: "reviewer-1",
      occurred_at: "2026-09-15T10:00:00Z",
    },
  ];
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("CaseDetail", () => {
  it("shows the case status, incident type, and history", async () => {
    await loginAndReachCase();

    expect(screen.getByText("Under review")).toBeInTheDocument();
    expect(screen.getByText(/MISSING CHILD/)).toBeInTheDocument();
    expect(screen.getByText("CASE CREATED")).toBeInTheDocument();
  });

  it("only offers the transitions the domain allows from the current status", async () => {
    await loginAndReachCase();

    expect(screen.getByRole("button", { name: "Mark Verified" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Mark Rejected" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Mark Active" })).not.toBeInTheDocument();
  });

  it("verifies a case via the shorthand endpoint and refreshes its status", async () => {
    const user = await loginAndReachCase();

    await user.click(screen.getByRole("button", { name: "Mark Verified" }));

    await waitFor(() => {
      expect(screen.getByText("Verified")).toBeInTheDocument();
    });
    expect(screen.getByRole("status")).toHaveTextContent("Case moved to Verified.");
    // Now that the case is VERIFIED, the next actions change accordingly.
    expect(screen.getByRole("button", { name: "Mark Active" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Mark Verified" })).not.toBeInTheDocument();
  });

  it("lists a linked report's attachments and fetches a download link on demand", async () => {
    const user = await loginAndReachCase();

    await user.click(screen.getByRole("button", { name: "Show attachments" }));
    await screen.findByText(/image\/jpeg/);

    await user.click(screen.getByRole("button", { name: "Get download link" }));

    const link = await screen.findByRole("link", { name: "Open" });
    expect(link).toHaveAttribute("href", "https://storage.example/signed");
  });

  it("creates an alert from a verified case and navigates to its preview", async () => {
    caseData = { ...caseData, status: "VERIFIED" };
    const user = await loginAndReachCase();

    await user.type(screen.getByLabelText("Target geography"), "Douala, Bonamoussadi");
    await user.type(screen.getByLabelText("Safe description"), "Last seen wearing a red shirt.");
    await user.click(screen.getByRole("button", { name: "Create alert" }));

    await screen.findByRole("heading", { name: `Alert ${ALERT_ID.slice(0, 8)}` });
  });

  it("does not offer alert creation before a case is verified", async () => {
    await loginAndReachCase();

    expect(screen.queryByRole("button", { name: "Create alert" })).not.toBeInTheDocument();
  });
});
