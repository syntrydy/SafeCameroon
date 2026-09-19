import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "../../App";
import type { Consumer } from "../../api/consumers";
import type { DeliveryPreference, Subscription, SubscriptionRule } from "../../api/subscriptions";
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

const CONSUMER: Consumer = {
  consumer_id: "eeeeeeee-5555-5555-5555-555555555555",
  name: "Douala Police",
  consumer_type: "ORGANIZATION",
};

let subscriptions: Subscription[];
let deliveryPreference: DeliveryPreference | null;

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string, init?: RequestInit) => {
      const url = new URL(input, "http://localhost");
      const method = init?.method ?? "GET";

      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === `/v1/consumers/${CONSUMER.consumer_id}` && method === "GET") {
        return Promise.resolve(jsonResponse(200, CONSUMER));
      }
      if (url.pathname === "/v1/consumers" && method === "POST") {
        const body = JSON.parse(init?.body as string) as { name: string; consumer_type: string };
        return Promise.resolve(
          jsonResponse(201, { consumer_id: CONSUMER.consumer_id, name: body.name, consumer_type: body.consumer_type }),
        );
      }
      if (url.pathname === `/v1/consumers/${CONSUMER.consumer_id}/subscriptions` && method === "GET") {
        return Promise.resolve(jsonResponse(200, subscriptions));
      }
      if (url.pathname === "/v1/subscriptions" && method === "GET") {
        return Promise.resolve(jsonResponse(200, subscriptions));
      }
      if (url.pathname === "/v1/subscriptions" && method === "POST") {
        const body = JSON.parse(init?.body as string) as { rules: SubscriptionRule[] };
        const created: Subscription = {
          subscription_id: "ffffffff-6666-6666-6666-666666666666",
          consumer_id: CONSUMER.consumer_id,
          version: 1,
          rules: body.rules,
        };
        subscriptions = [...subscriptions, created];
        return Promise.resolve(jsonResponse(201, created));
      }
      if (/\/v1\/subscriptions\/.+/.test(url.pathname) && method === "PUT") {
        const body = JSON.parse(init?.body as string) as { rules: SubscriptionRule[] };
        const id = url.pathname.split("/").pop()!;
        const updated: Subscription = {
          subscription_id: id,
          consumer_id: CONSUMER.consumer_id,
          version: 2,
          rules: body.rules,
        };
        subscriptions = subscriptions.map((s) => (s.subscription_id === id ? updated : s));
        return Promise.resolve(jsonResponse(200, updated));
      }
      if (url.pathname === `/v1/consumers/${CONSUMER.consumer_id}/delivery-preference` && method === "GET") {
        if (!deliveryPreference) {
          return Promise.resolve(
            jsonResponse(404, {
              error: {
                code: "DELIVERY_PREFERENCE_NOT_FOUND",
                message: "No delivery preference is set for this consumer.",
                request_id: "req-1",
              },
            }),
          );
        }
        return Promise.resolve(jsonResponse(200, deliveryPreference));
      }
      if (url.pathname === `/v1/consumers/${CONSUMER.consumer_id}/delivery-preference` && method === "PUT") {
        const body = JSON.parse(init?.body as string) as DeliveryPreference;
        deliveryPreference = { consumer_id: CONSUMER.consumer_id, strategy: body.strategy, channels: body.channels };
        return Promise.resolve(jsonResponse(200, deliveryPreference));
      }
      throw new Error(`unexpected fetch: ${method} ${url.pathname}`);
    }),
  );
}

async function loginAndReachSubscriptions() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={["/subscriptions"]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: "Subscriptions" });
  return user;
}

beforeEach(() => {
  subscriptions = [];
  deliveryPreference = null;
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Subscriptions", () => {
  it("lists every subscription and can jump to its consumer", async () => {
    subscriptions = [
      {
        subscription_id: "ffffffff-6666-6666-6666-666666666666",
        consumer_id: CONSUMER.consumer_id,
        version: 1,
        rules: [{ rule: "INCIDENT_TYPE", values: ["MISSING_CHILD"] }],
      },
    ];
    const user = await loginAndReachSubscriptions();

    await screen.findByText("Incident type: MISSING_CHILD");
    expect(screen.getByText(CONSUMER.consumer_id)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "View consumer" }));

    await screen.findByText("Douala Police");
  });

  it("looks up an existing consumer by id and shows it has no subscriptions yet", async () => {
    const user = await loginAndReachSubscriptions();

    await user.type(screen.getByLabelText("Consumer id"), CONSUMER.consumer_id);
    await user.click(screen.getByRole("button", { name: "Load" }));

    await screen.findByText("Douala Police");
    expect(screen.getByText("No delivery preference is set yet.")).toBeInTheDocument();
  });

  it("registers a new consumer and reaches its (empty) subscriptions", async () => {
    const user = await loginAndReachSubscriptions();

    await user.type(screen.getByLabelText("Name"), "Douala Police");
    await user.click(screen.getByRole("button", { name: "Create consumer" }));

    await screen.findByText("Douala Police");
  });

  it("creates a subscription with an incident-type rule", async () => {
    const user = await loginAndReachSubscriptions();
    await user.type(screen.getByLabelText("Consumer id"), CONSUMER.consumer_id);
    await user.click(screen.getByRole("button", { name: "Load" }));
    await screen.findByText("Douala Police");

    await user.click(screen.getByRole("button", { name: "Add subscription" }));
    await user.click(screen.getByRole("checkbox", { name: "MISSING CHILD" }));
    await user.click(screen.getByRole("button", { name: "Create subscription" }));

    await screen.findByText("Incident type: MISSING_CHILD");
  });

  it("edits an existing subscription's rules", async () => {
    subscriptions = [
      {
        subscription_id: "ffffffff-6666-6666-6666-666666666666",
        consumer_id: CONSUMER.consumer_id,
        version: 1,
        rules: [{ rule: "GEOGRAPHY", areas: ["Douala"] }],
      },
    ];
    const user = await loginAndReachSubscriptions();
    await user.type(screen.getByLabelText("Consumer id"), CONSUMER.consumer_id);
    await user.click(screen.getByRole("button", { name: "Load" }));
    await screen.findByText("Geography: Douala");

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.click(screen.getByRole("button", { name: "Remove Douala" }));
    await user.type(screen.getByLabelText("Rule 1 area"), "Yaounde");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    await waitFor(() => {
      expect(screen.getByText("Geography: Yaounde")).toBeInTheDocument();
    });
  });

  it("adds multiple areas to a geography rule", async () => {
    subscriptions = [];
    const user = await loginAndReachSubscriptions();
    await user.type(screen.getByLabelText("Consumer id"), CONSUMER.consumer_id);
    await user.click(screen.getByRole("button", { name: "Load" }));
    await screen.findByText("Douala Police");

    await user.click(screen.getByRole("button", { name: "Add subscription" }));
    await user.selectOptions(screen.getByLabelText("Rule 1 type"), "GEOGRAPHY");
    await user.type(screen.getByLabelText("Rule 1 area"), "Douala");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.type(screen.getByLabelText("Rule 1 area"), "Yaounde");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(screen.getByRole("button", { name: "Create subscription" }));

    await screen.findByText("Geography: Douala, Yaounde");
  });

  it("sets a delivery preference", async () => {
    const user = await loginAndReachSubscriptions();
    await user.type(screen.getByLabelText("Consumer id"), CONSUMER.consumer_id);
    await user.click(screen.getByRole("button", { name: "Load" }));
    await screen.findByText("No delivery preference is set yet.");

    await user.type(screen.getByLabelText("Address 1"), "+237600000000");
    await user.click(screen.getByRole("button", { name: "Save delivery preference" }));

    await waitFor(() => {
      expect(screen.queryByText("No delivery preference is set yet.")).not.toBeInTheDocument();
    });
  });

  it("shows an error when looking up an unknown consumer", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((input: string, init?: RequestInit) => {
        const url = new URL(input, "http://localhost");
        if (url.pathname === "/v1/auth/google") return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
        if (url.pathname === "/v1/subscriptions" && (init?.method ?? "GET") === "GET") {
          return Promise.resolve(jsonResponse(200, []));
        }
        if (url.pathname.startsWith("/v1/consumers/")) {
          return Promise.resolve(
            jsonResponse(404, {
              error: { code: "CONSUMER_NOT_FOUND", message: "No consumer exists with the given id.", request_id: "req-2" },
            }),
          );
        }
        throw new Error(`unexpected fetch: ${init?.method ?? "GET"} ${url.pathname}`);
      }),
    );
    const user = await loginAndReachSubscriptions();

    await user.type(screen.getByLabelText("Consumer id"), "nonexistent");
    await user.click(screen.getByRole("button", { name: "Load" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("No consumer exists with the given id.");
  });
});
