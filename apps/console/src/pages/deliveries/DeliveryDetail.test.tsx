import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "../../App";
import type { DeliveryDetail as DeliveryDetailData } from "../../api/deliveries";
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

const DELIVERY_ID = "11111111-7777-7777-7777-777777777777";

let delivery: DeliveryDetailData;

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string) => {
      const url = new URL(input, "http://localhost");
      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === `/v1/deliveries/${DELIVERY_ID}`) {
        return Promise.resolve(jsonResponse(200, delivery));
      }
      throw new Error(`unexpected fetch: GET ${url.pathname}`);
    }),
  );
}

async function loginAndReachDelivery() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={[`/deliveries/${DELIVERY_ID}`]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: `Delivery ${DELIVERY_ID.slice(0, 8)}` });
  return user;
}

beforeEach(() => {
  delivery = {
    delivery_id: DELIVERY_ID,
    alert_id: "dddddddd-4444-4444-4444-444444444444",
    consumer_id: "eeeeeeee-5555-5555-5555-555555555555",
    channel: "WHATSAPP",
    endpoint_address: "+237600000000",
    tier: 1,
    status: "RETRYING",
    attempt_count: 2,
    max_attempts: 3,
    version: 3,
    attempts: [
      { attempt_number: 1, outcome: "FAILED", provider_message_id: null, retryable: true, failure_reason: "Provider timeout" },
      { attempt_number: 2, outcome: "SENT", provider_message_id: "wa-msg-123", retryable: null, failure_reason: null },
    ],
  };
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("DeliveryDetail", () => {
  it("shows delivery metadata and the full attempt history", async () => {
    await loginAndReachDelivery();

    expect(screen.getByText("+237600000000")).toBeInTheDocument();
    expect(screen.getByText("RETRYING")).toBeInTheDocument();
    expect(screen.getByText("Provider timeout (retryable)")).toBeInTheDocument();
    expect(screen.getByText("Provider message id: wa-msg-123")).toBeInTheDocument();
  });
});
