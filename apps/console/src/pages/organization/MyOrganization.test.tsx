import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "../../App";
import type { DeliveryPreference } from "../../api/subscriptions";
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

const ORGANIZATION_ID = "83b515a8-7441-4db7-9ae2-05a278142078";
const CONSUMER_ID = "a2e99adb-0e34-4d2a-a961-ab8f04c39586";

const LOGIN_RESPONSE = {
  token: "a-session-token",
  expires_in_seconds: 3600,
  reviewer_id: "reviewer-1",
  email: "reviewer@example.test",
  role: "ORG_ADMIN",
  organization_id: ORGANIZATION_ID,
};

// A delivery preference already saved on the backend -- the bug this test
// guards is that the loaded value never reaches the form: it mounts before
// this fetch resolves and its internal state never re-syncs to the `initial`
// prop that arrives afterward.
const SAVED_DELIVERY_PREFERENCE: DeliveryPreference = {
  consumer_id: CONSUMER_ID,
  strategy: "ALL",
  channels: [{ channel: "EMAIL", address: "org@example.org" }],
};

// Gates the mocked GET .../delivery-preference response so it resolves
// after the initial render, matching how the real network round trip
// always lands after the component has already mounted once.
let deliveryPreferenceGate: Promise<void> = Promise.resolve();

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string, init?: RequestInit) => {
      const url = new URL(input, "http://localhost");
      const method = init?.method ?? "GET";

      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === `/v1/organizations/${ORGANIZATION_ID}` && method === "GET") {
        return Promise.resolve(
          jsonResponse(200, {
            organization_id: ORGANIZATION_ID,
            name: "Save the Children",
            description: "Global emergency response.",
            location: "Worldwide",
            contact: "+237600000000",
            verified_incident_types: ["MISSING_CHILD"],
            verified_alert_visibilities: ["COMMUNITY"],
            consumer_id: CONSUMER_ID,
            is_active: true,
          }),
        );
      }
      if (url.pathname === `/v1/organizations/${ORGANIZATION_ID}/members` && method === "GET") {
        return Promise.resolve(jsonResponse(200, []));
      }
      if (url.pathname === `/v1/consumers/${CONSUMER_ID}/subscriptions` && method === "GET") {
        return Promise.resolve(jsonResponse(200, []));
      }
      if (url.pathname === `/v1/consumers/${CONSUMER_ID}/delivery-preference` && method === "GET") {
        return deliveryPreferenceGate.then(() => jsonResponse(200, SAVED_DELIVERY_PREFERENCE));
      }
      throw new Error(`unexpected fetch: ${method} ${url.pathname}`);
    }),
  );
}

async function loginAndReachMyOrganization() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={["/my-organization"]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: "Save the Children" });
  return user;
}

beforeEach(() => {
  deliveryPreferenceGate = Promise.resolve();
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("MyOrganization", () => {
  it("renders the org's already-saved delivery preference once it loads, not the form's blank default", async () => {
    let releaseDeliveryPreferenceGate!: () => void;
    deliveryPreferenceGate = new Promise((resolve) => {
      releaseDeliveryPreferenceGate = resolve;
    });
    await loginAndReachMyOrganization();
    releaseDeliveryPreferenceGate();

    // The stale-mount bug shows the form's hardcoded blank default
    // (WHATSAPP / empty address) instead of the already-saved preference.
    const addressField = await screen.findByLabelText("Address 1");
    expect(addressField).toHaveValue("org@example.org");
    expect(screen.getByLabelText("Channel 1")).toHaveValue("EMAIL");
  });
});
