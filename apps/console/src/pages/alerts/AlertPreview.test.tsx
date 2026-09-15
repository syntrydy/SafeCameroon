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
};

const ALERT_ID = "dddddddd-4444-4444-4444-444444444444";

let alert: Alert;

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string, init?: RequestInit) => {
      const url = new URL(input, "http://localhost");
      const method = init?.method ?? "GET";

      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === `/v1/alerts/${ALERT_ID}` && method === "GET") {
        return Promise.resolve(jsonResponse(200, alert));
      }
      if (url.pathname === `/v1/alerts/${ALERT_ID}/cancel` && method === "POST") {
        alert = { ...alert, status: "CANCELLED", version: alert.version + 1 };
        return Promise.resolve(jsonResponse(200, alert));
      }
      throw new Error(`unexpected fetch: ${method} ${url.pathname}`);
    }),
  );
}

async function loginAndReachAlert() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={[`/alerts/${ALERT_ID}`]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: `Alert ${ALERT_ID.slice(0, 8)}` });
  return user;
}

beforeEach(() => {
  alert = {
    alert_id: ALERT_ID,
    case_id: "aaaaaaaa-1111-1111-1111-111111111111",
    policy_id: "MISSING_CHILD_COMMUNITY",
    policy_version: 1,
    incident_type: "MISSING_CHILD",
    severity: "HIGH",
    visibility: "COMMUNITY",
    trigger: "CASE_VERIFIED",
    target_geography: "Douala, Bonamoussadi",
    status: "ACTIVE",
    fields: [
      { field: "INCIDENT_CATEGORY", value: "MISSING_CHILD" },
      { field: "SAFE_DESCRIPTION", value: "Last seen wearing a red shirt." },
    ],
    version: 1,
  };
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("AlertPreview", () => {
  it("shows the alert's safe projection", async () => {
    await loginAndReachAlert();

    expect(screen.getByText("Douala, Bonamoussadi")).toBeInTheDocument();
    expect(screen.getByText("Last seen wearing a red shirt.")).toBeInTheDocument();
    expect(screen.getByText("Active")).toBeInTheDocument();
  });

  it("cancels an active alert", async () => {
    const user = await loginAndReachAlert();

    await user.click(screen.getByRole("button", { name: "Cancel alert" }));

    await waitFor(() => {
      expect(screen.getByText("Cancelled")).toBeInTheDocument();
    });
    expect(screen.getByRole("status")).toHaveTextContent("Alert cancelled.");
    expect(screen.queryByRole("button", { name: "Cancel alert" })).not.toBeInTheDocument();
  });
});
