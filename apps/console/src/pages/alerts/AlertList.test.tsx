import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "../../App";
import type { Alert } from "../../api/alerts";
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

const ACTIVE_ALERT: Alert = {
  alert_id: "dddddddd-4444-4444-4444-444444444444",
  case_id: "aaaaaaaa-1111-1111-1111-111111111111",
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
};

let alerts: Alert[];

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string) => {
      const url = new URL(input, "http://localhost");
      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === "/v1/alerts") {
        const status = url.searchParams.get("status");
        const filtered = status ? alerts.filter((alert) => alert.status === status) : alerts;
        return Promise.resolve(jsonResponse(200, filtered));
      }
      throw new Error(`unexpected fetch: GET ${url.pathname}`);
    }),
  );
}

async function loginAndReachAlerts() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={["/alerts"]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: "Alerts" });
  return user;
}

beforeEach(() => {
  alerts = [{ ...ACTIVE_ALERT }];
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("AlertList", () => {
  it("lists active alerts by default", async () => {
    await loginAndReachAlerts();

    expect(await screen.findByText("Douala, Bonamoussadi")).toBeInTheDocument();
  });

  it("refetches with the selected status filter", async () => {
    const user = await loginAndReachAlerts();
    await screen.findByText("Douala, Bonamoussadi");

    await user.selectOptions(screen.getByLabelText(/Status:/), "CANCELLED");

    await waitFor(() => {
      expect(screen.getByText("No alerts match this filter.")).toBeInTheDocument();
    });
  });

  it("links each alert to its preview screen", async () => {
    await loginAndReachAlerts();

    const link = await screen.findByRole("link", { name: ACTIVE_ALERT.alert_id.slice(0, 8) });
    expect(link).toHaveAttribute("href", `/alerts/${ACTIVE_ALERT.alert_id}`);
  });
});
