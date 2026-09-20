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
  role: "PLATFORM_ADMIN",
  organization_id: null,
};

const CASE_ID = "aaaaaaaa-1111-1111-1111-111111111111";
const REPORT_ID = "bbbbbbbb-2222-2222-2222-222222222222";
const ALERT_ID = "dddddddd-4444-4444-4444-444444444444";

let caseData: Case;
let events: CaseEvent[];
// Gates the mocked GET .../extractions response, which the case detail
// page's auto-apply-on-load kicks off immediately. Resolved instantly by
// default (every other test); one test below replaces it with a promise it
// controls, so it can type into a field *before* the auto-apply round trip
// is allowed to complete -- otherwise, since these mocked fetches resolve
// far faster than simulated keystrokes, there's no reliable way to observe
// "typed before the suggestion landed" versus "typed after."
let extractionsListGate: Promise<void> = Promise.resolve();

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
      if (url.pathname === `/v1/reports/${REPORT_ID}` && method === "GET") {
        return Promise.resolve(
          jsonResponse(200, {
            report_id: REPORT_ID,
            source_channel: "WEB",
            status: "UNDER_REVIEW",
            raw_content: "A child went missing near the central market.",
            received_at: "2026-09-15T09:00:00Z",
            reported_incident_type: "MISSING_CHILD",
          }),
        );
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
      if (url.pathname === "/v1/subscriptions/geography-areas" && method === "GET") {
        return Promise.resolve(jsonResponse(200, ["Bonamoussadi", "Douala"]));
      }
      if (url.pathname === `/v1/reports/${REPORT_ID}/extractions` && method === "GET") {
        return extractionsListGate.then(() => jsonResponse(200, []));
      }
      if (url.pathname === `/v1/reports/${REPORT_ID}/extractions` && method === "POST") {
        return Promise.resolve(
          jsonResponse(201, {
            report_id: REPORT_ID,
            requested_by: "reviewer-1",
            provider: "OPENROUTER",
            model: "openai/gpt-4o-mini",
            prompt_version: "v1",
            fields: {
              person_description: "an 8-year-old girl in a blue school uniform",
              age: "8 years old",
              time: "around 3:15 PM",
              place: "Carrefour Bonamoussadi, Douala",
              incident_category: "Did not return from school",
              vehicle_details: null,
              contact_request: null,
            },
          }),
        );
      }
      if (url.pathname === `/v1/cases/${CASE_ID}/alert-description-suggestions` && method === "POST") {
        return Promise.resolve(
          jsonResponse(201, {
            description_en: "Formal: last seen wearing a red shirt near the market.",
            description_fr: "Formel : vue pour la derniere fois en chemise rouge pres du marche.",
            provider: "OPENROUTER",
            model: "openai/gpt-4o",
            prompt_version: "v1",
          }),
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
  extractionsListGate = Promise.resolve();
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
    // (Scoped by text, not role="status" alone -- verifying also auto-applies
    // the linked report's AI extraction, which renders its own status text.)
    expect(screen.getByText("Case moved to Verified.")).toBeInTheDocument();
    // Now that the case is VERIFIED, the next actions change accordingly.
    expect(screen.getByRole("button", { name: "Mark Active" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Mark Verified" })).not.toBeInTheDocument();
  });

  it("shows the linked report's text", async () => {
    await loginAndReachCase();

    await screen.findByText("A child went missing near the central market.");
  });

  it("lists a linked report's attachments and fetches a download link on demand", async () => {
    const user = await loginAndReachCase();

    await user.click(screen.getByRole("button", { name: "Show attachments" }));
    await screen.findByText(/image\/jpeg/);

    await user.click(screen.getByRole("button", { name: "Get download link" }));

    const link = await screen.findByRole("link", { name: "Open" });
    expect(link).toHaveAttribute("href", "https://storage.example/signed");
  });

  it("renders an inline image preview for an image attachment once its download link is fetched", async () => {
    const user = await loginAndReachCase();

    await user.click(screen.getByRole("button", { name: "Show attachments" }));
    await screen.findByText(/image\/jpeg/);
    await user.click(screen.getByRole("button", { name: "Get download link" }));

    const preview = await screen.findByAltText("Attachment preview");
    expect(preview).toHaveAttribute("src", "https://storage.example/signed");
  });

  it("creates an alert from a verified case and navigates to its preview", async () => {
    caseData = { ...caseData, status: "VERIFIED" };
    const user = await loginAndReachCase();

    // Target geography is picked from real subscribed areas, not typed
    // free text -- typing filters the known-areas list, and Enter selects
    // the top match.
    const areaInput = screen.getByLabelText("Target geography");
    await user.type(areaInput, "Douala{Enter}");
    await user.type(areaInput, "Bonamoussadi{Enter}");
    await user.type(screen.getByLabelText("Alert description (EN)"), "Last seen wearing a red shirt.");
    await user.type(screen.getByLabelText("Alert description (FR)"), "Vu pour la derniere fois en chemise rouge.");
    await user.click(screen.getByRole("button", { name: "Create alert" }));

    await screen.findByRole("heading", { name: `Alert ${ALERT_ID.slice(0, 8)}` });
  });

  it("requires at least one target geography area before creating an alert", async () => {
    caseData = { ...caseData, status: "VERIFIED" };
    // Gated so the case's own auto-apply (PR #189) never resolves and
    // pre-selects an area on its own -- this test wants a genuinely empty
    // selection.
    extractionsListGate = new Promise(() => {});
    const user = await loginAndReachCase();

    await user.type(screen.getByLabelText("Alert description (EN)"), "Last seen wearing a red shirt.");
    await user.type(screen.getByLabelText("Alert description (FR)"), "Vu pour la derniere fois en chemise rouge.");
    await user.click(screen.getByRole("button", { name: "Create alert" }));

    expect(
      await screen.findByText("Select at least one area -- this is who gets notified."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: `Alert ${ALERT_ID.slice(0, 8)}` })).not.toBeInTheDocument();
  });

  it("does not add a typed area that matches no known subscribed area", async () => {
    caseData = { ...caseData, status: "VERIFIED" };
    const user = await loginAndReachCase();

    await user.type(screen.getByLabelText("Target geography"), "Nowhereville{Enter}");

    expect(screen.queryByRole("button", { name: "Remove Nowhereville" })).not.toBeInTheDocument();
  });

  it("auto-applies the first linked report's AI extraction into the create-alert form without overwriting typed fields", async () => {
    caseData = { ...caseData, status: "VERIFIED" };
    let releaseExtractionsListGate!: () => void;
    extractionsListGate = new Promise((resolve) => {
      releaseExtractionsListGate = resolve;
    });
    const user = await loginAndReachCase();

    // The auto-apply round trip is gated (see extractionsListGate) until
    // released below, so this is guaranteed to land in state first.
    await user.type(screen.getByLabelText("Alert description (EN)"), "Reviewer's own draft.");
    releaseExtractionsListGate();

    expect(
      await screen.findByText("Filled in from the report's AI suggestion -- review every value before sending."),
    ).toBeInTheDocument();
    // Target geography wasn't touched, so it's auto-selected from whichever
    // known subscribed areas are substrings of the extraction's `place`
    // ("Carrefour Bonamoussadi, Douala" contains both).
    expect(screen.getByRole("button", { name: "Remove Douala" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove Bonamoussadi" })).toBeInTheDocument();
    // The EN description was already typed into, so neither the composed
    // suggestion nor the auto-generate step it would otherwise trigger ever
    // runs -- FR stays untouched too.
    expect(screen.getByLabelText("Alert description (EN)")).toHaveValue("Reviewer's own draft.");
    expect(screen.getByLabelText("Alert description (FR)")).toHaveValue("");
  });

  it("generates a formal EN/FR description pair from the drafted English text", async () => {
    caseData = { ...caseData, status: "VERIFIED" };
    const user = await loginAndReachCase();

    await user.type(screen.getByLabelText("Alert description (EN)"), "girl in red shirt near market");
    await user.click(screen.getByRole("button", { name: "Generate formal description" }));

    await waitFor(() => {
      expect(screen.getByLabelText("Alert description (EN)")).toHaveValue(
        "Formal: last seen wearing a red shirt near the market.",
      );
    });
    expect(screen.getByLabelText("Alert description (FR)")).toHaveValue(
      "Formel : vue pour la derniere fois en chemise rouge pres du marche.",
    );
    expect(screen.getByText("openrouter/openai/gpt-4o")).toBeInTheDocument();
  });

  it("auto-generates a formal EN+FR description from the AI extraction when untouched", async () => {
    caseData = { ...caseData, status: "VERIFIED" };
    await loginAndReachCase();

    expect(
      await screen.findByText("Filled in from the report's AI suggestion -- review every value before sending."),
    ).toBeInTheDocument();
    // The mechanical field join is only ever a fleeting fallback: applying
    // an extraction immediately triggers the same formal-description
    // generation the manual button does, landing on a real sentence in
    // both languages without an extra click.
    await waitFor(() => {
      expect(screen.getByLabelText("Alert description (EN)")).toHaveValue(
        "Formal: last seen wearing a red shirt near the market.",
      );
    });
    expect(screen.getByLabelText("Alert description (FR)")).toHaveValue(
      "Formel : vue pour la derniere fois en chemise rouge pres du marche.",
    );
  });

  it("does not offer alert creation before a case is verified", async () => {
    await loginAndReachCase();

    expect(screen.queryByRole("button", { name: "Create alert" })).not.toBeInTheDocument();
  });
});
