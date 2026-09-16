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
};

/** Routes a mocked fetch by method + path so login (and the review queue it
 * lands on) can be exercised without a real backend. */
function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string, init?: RequestInit) => {
      const url = new URL(input, "http://localhost");
      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === "/v1/auth/logout") {
        return Promise.resolve(new Response(null, { status: 204 }));
      }
      if (url.pathname === "/v1/reports" && (init?.method ?? "GET") === "GET") {
        return Promise.resolve(jsonResponse(200, []));
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
    expect(screen.getByText(`Reviewer ${LOGIN_RESPONSE.reviewer_id}`)).toBeInTheDocument();
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
});
