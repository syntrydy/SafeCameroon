import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "../../App";
import type { Delivery } from "../../api/deliveries";
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

const ALERT_ID = "dddddddd-4444-4444-4444-444444444444";
const DELIVERY: Delivery = {
  delivery_id: "11111111-7777-7777-7777-777777777777",
  alert_id: ALERT_ID,
  consumer_id: "eeeeeeee-5555-5555-5555-555555555555",
  channel: "WHATSAPP",
  endpoint_address: "+237600000000",
  tier: 1,
  status: "DELIVERED",
  attempt_count: 1,
  max_attempts: 3,
  version: 2,
};

let deliveries: Delivery[];

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string) => {
      const url = new URL(input, "http://localhost");
      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === `/v1/alerts/${ALERT_ID}/deliveries`) {
        return Promise.resolve(jsonResponse(200, deliveries));
      }
      throw new Error(`unexpected fetch: GET ${url.pathname}`);
    }),
  );
}

async function loginAndReachDeliveries() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={[`/alerts/${ALERT_ID}/deliveries`]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: `Deliveries for alert ${ALERT_ID.slice(0, 8)}` });
  return user;
}

beforeEach(() => {
  deliveries = [{ ...DELIVERY }];
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("DeliveryList", () => {
  it("lists deliveries planned for the alert", async () => {
    await loginAndReachDeliveries();

    expect(screen.getByText("WHATSAPP")).toBeInTheDocument();
    expect(screen.getByText("DELIVERED")).toBeInTheDocument();
    expect(screen.getByText("1 / 3")).toBeInTheDocument();
  });

  it("shows an empty state when no deliveries have been planned", async () => {
    deliveries = [];
    await loginAndReachDeliveries();

    expect(await screen.findByText("No deliveries have been planned for this alert.")).toBeInTheDocument();
  });

  it("links each delivery to its detail screen", async () => {
    await loginAndReachDeliveries();

    const link = await screen.findByRole("link", { name: DELIVERY.delivery_id.slice(0, 8) });
    expect(link).toHaveAttribute("href", `/deliveries/${DELIVERY.delivery_id}`);
  });
});
