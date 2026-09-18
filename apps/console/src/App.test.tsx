import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "./App";
import { AuthProvider } from "./auth/AuthContext";

// The real component loads Google's own script and isn't meaningfully
// testable in jsdom; stand in a plain button so tests can drive the same
// onCredential callback the real widget would invoke.
vi.mock("./auth/GoogleSignInButton", () => ({
  GoogleSignInButton: ({ onCredential }: { onCredential: (idToken: string) => void }) => (
    <button type="button" onClick={() => onCredential("fake-google-id-token")}>
      Fake Google Sign-In
    </button>
  ),
}));

function renderApp(initialPath = "/") {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );
}

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

const LOGIN_RESPONSE = {
  token: "a-session-token",
  expires_in_seconds: 3600,
  reviewer_id: "22222222-2222-2222-2222-222222222222",
  email: "reviewer@example.test",
  role: "MEMBER",
};

const CONSUMER_ID = "77777777-7777-7777-7777-777777777777";

/** Routes a mocked fetch by method + path so login (and the review queue it
 * lands on) can be exercised without a real backend. `organizationId` stands
 * in the org admin's own org for the /my-organization routes; when set, its
 * organization is linked to `CONSUMER_ID` (mirrors every organization
 * created since migration 0025). */
function stubBackend(role: string = "MEMBER", organizationId: string | null = null) {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string, init?: RequestInit) => {
      const url = new URL(input, "http://localhost");
      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(
          jsonResponse(200, { ...LOGIN_RESPONSE, role, organization_id: organizationId }),
        );
      }
      if (url.pathname === "/v1/auth/logout") {
        return Promise.resolve(new Response(null, { status: 204 }));
      }
      if (url.pathname === "/v1/reports" && (init?.method ?? "GET") === "GET") {
        return Promise.resolve(jsonResponse(200, []));
      }
      if (url.pathname === "/v1/organizations" && (init?.method ?? "GET") === "GET") {
        return Promise.resolve(jsonResponse(200, []));
      }
      if (
        organizationId &&
        url.pathname === `/v1/organizations/${organizationId}` &&
        (init?.method ?? "GET") === "GET"
      ) {
        return Promise.resolve(
          jsonResponse(200, {
            organization_id: organizationId,
            name: "Douala Police",
            verified_incident_types: [],
            verified_alert_visibilities: [],
            consumer_id: CONSUMER_ID,
          }),
        );
      }
      if (
        organizationId &&
        url.pathname === `/v1/organizations/${organizationId}/members` &&
        (init?.method ?? "GET") === "GET"
      ) {
        return Promise.resolve(jsonResponse(200, []));
      }
      if (url.pathname === `/v1/consumers/${CONSUMER_ID}/subscriptions` && (init?.method ?? "GET") === "GET") {
        return Promise.resolve(jsonResponse(200, []));
      }
      if (url.pathname === `/v1/consumers/${CONSUMER_ID}/delivery-preference` && (init?.method ?? "GET") === "GET") {
        return Promise.resolve(
          jsonResponse(404, {
            error: {
              code: "DELIVERY_PREFERENCE_NOT_FOUND",
              message: "No delivery preference is configured for this consumer.",
              request_id: "88888888-8888-8888-8888-888888888888",
            },
          }),
        );
      }
      if (url.pathname === `/v1/consumers/${CONSUMER_ID}/delivery-preference` && init?.method === "PUT") {
        const body = init?.body ? JSON.parse(String(init.body)) : {};
        return Promise.resolve(jsonResponse(200, { strategy: body.strategy, channels: body.channels }));
      }
      if (url.pathname === "/v1/organizations" && init?.method === "POST") {
        const body = init?.body ? JSON.parse(String(init.body)) : {};
        return Promise.resolve(
          jsonResponse(201, {
            organization_id: "66666666-6666-6666-6666-666666666666",
            name: body.name ?? "",
            verified_incident_types: [],
            verified_alert_visibilities: [],
          }),
        );
      }
      if (
        /^\/v1\/organizations\/[0-9a-fA-F-]+\/members$/.test(url.pathname) &&
        (init?.method ?? "GET") === "GET"
      ) {
        return Promise.resolve(jsonResponse(200, []));
      }
      if (url.pathname === "/v1/auth/register" && init?.method === "POST") {
        return Promise.resolve(
          jsonResponse(201, {
            reviewer_id: "44444444-4444-4444-4444-444444444444",
            email: "new-member@example.test",
            role: "MEMBER",
            organization_id: organizationId,
          }),
        );
      }
      throw new Error(`unexpected fetch: ${init?.method ?? "GET"} ${url.pathname}`);
    }),
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("console auth flow", () => {
  it("redirects an unauthenticated visitor to /login", () => {
    renderApp("/");

    expect(screen.getByRole("heading", { name: "Welcome back" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Fake Google Sign-In" })).toBeInTheDocument();
  });

  it("logs in with a verified Google credential and reaches the review queue", async () => {
    stubBackend();
    const user = userEvent.setup();
    renderApp("/login");

    await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Review queue" })).toBeInTheDocument();
    });
    expect(screen.getByText(`Reviewer ${LOGIN_RESPONSE.email}`)).toBeInTheDocument();
  });

  it("shows the backend's error message and request id when the account isn't a registered reviewer", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(403, {
          error: {
            code: "REVIEWER_NOT_REGISTERED",
            message: "This Google account is not registered as a reviewer.",
            request_id: "33333333-3333-3333-3333-333333333333",
          },
        }),
      ),
    );
    const user = userEvent.setup();
    renderApp("/login");

    await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("This Google account is not registered as a reviewer.");
    expect(alert).toHaveTextContent("33333333-3333-3333-3333-333333333333");
  });

  it("signs out back to the login screen", async () => {
    stubBackend();
    const user = userEvent.setup();
    renderApp("/login");

    await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
    await screen.findByRole("button", { name: "Sign out" });

    await user.click(screen.getByRole("button", { name: "Sign out" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Fake Google Sign-In" })).toBeInTheDocument();
    });
  });

  it("hides the Organizations link and redirects away from it for a non-admin reviewer", async () => {
    stubBackend("MEMBER");
    const user = userEvent.setup();
    renderApp("/login");

    await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
    await screen.findByRole("heading", { name: "Review queue" });

    expect(screen.queryByRole("link", { name: "Organizations" })).not.toBeInTheDocument();
  });

  it("shows the Organizations link and an Admin label for a platform admin", async () => {
    stubBackend("PLATFORM_ADMIN");
    const user = userEvent.setup();
    renderApp("/login");

    await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
    await screen.findByRole("heading", { name: "Review queue" });

    expect(screen.getByText(`Admin ${LOGIN_RESPONSE.email}`)).toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "Organizations" }));

    expect(await screen.findByRole("heading", { name: "Organizations" })).toBeInTheDocument();
  });

  it("hides the My organization link for a plain member", async () => {
    stubBackend("MEMBER");
    const user = userEvent.setup();
    renderApp("/login");

    await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
    await screen.findByRole("heading", { name: "Review queue" });

    expect(screen.queryByRole("link", { name: "My organization" })).not.toBeInTheDocument();
  });

  it("shows the My organization link for an org admin and lets them invite a member", async () => {
    const organizationId = "55555555-5555-5555-5555-555555555555";
    stubBackend("ORG_ADMIN", organizationId);
    const user = userEvent.setup();
    renderApp("/login");

    await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
    await screen.findByRole("heading", { name: "Review queue" });

    expect(screen.getByText(`Org admin ${LOGIN_RESPONSE.email}`)).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Organizations" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("link", { name: "My organization" }));

    expect(await screen.findByRole("heading", { name: "Douala Police" })).toBeInTheDocument();

    // The org's linked consumer's notification setup is reachable from the
    // same page: no delivery preference yet, and it can be set.
    expect(await screen.findByText("No delivery preference is set yet.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Address 1"), "+237600000001");
    await user.click(screen.getByRole("button", { name: "Save delivery preference" }));
    await waitFor(() => {
      expect(screen.queryByText("No delivery preference is set yet.")).not.toBeInTheDocument();
    });

    await user.type(screen.getByLabelText("Email"), "newbie@example.test");
    await user.click(screen.getByRole("button", { name: "Invite" }));

    await waitFor(() => {
      expect(screen.getByLabelText("Email")).toHaveValue("");
    });
  });

  it("lets a platform admin create an organization and invite its first org admin", async () => {
    stubBackend("PLATFORM_ADMIN");
    const user = userEvent.setup();
    renderApp("/login");

    await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
    await screen.findByRole("heading", { name: "Review queue" });

    await user.click(screen.getByRole("link", { name: "Organizations" }));
    await screen.findByRole("heading", { name: "Organizations" });

    await user.type(screen.getByLabelText("Name"), "Douala Police");
    await user.click(screen.getByRole("button", { name: "Create organization" }));

    await user.click(await screen.findByRole("button", { name: "View details" }));

    await user.type(screen.getByLabelText("Email"), "admin@douala-police.example");
    await user.click(screen.getByRole("button", { name: "Invite org admin" }));

    await waitFor(() => {
      expect(screen.getByLabelText("Email")).toHaveValue("");
    });
  });
});
