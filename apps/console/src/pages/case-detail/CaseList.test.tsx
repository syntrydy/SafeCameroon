import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { App } from "../../App";
import type { Case } from "../../api/cases";
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

const REPORTED_CASE: Case = {
  case_id: "aaaaaaaa-1111-1111-1111-111111111111",
  incident_type: "MISSING_CHILD",
  status: "REPORTED",
  report_ids: ["bbbbbbbb-2222-2222-2222-222222222222"],
  version: 1,
};

let cases: Case[];

function stubBackend() {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockImplementation((input: string) => {
      const url = new URL(input, "http://localhost");
      if (url.pathname === "/v1/auth/google") {
        return Promise.resolve(jsonResponse(200, LOGIN_RESPONSE));
      }
      if (url.pathname === "/v1/cases") {
        const status = url.searchParams.get("status");
        const filtered = status ? cases.filter((c) => c.status === status) : cases;
        return Promise.resolve(jsonResponse(200, filtered));
      }
      throw new Error(`unexpected fetch: GET ${url.pathname}`);
    }),
  );
}

async function loginAndReachCases() {
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={["/cases"]}>
      <AuthProvider>
        <App />
      </AuthProvider>
    </MemoryRouter>,
  );

  await user.click(screen.getByRole("button", { name: "Fake Google Sign-In" }));
  await screen.findByRole("heading", { name: "Cases" });
  return user;
}

beforeEach(() => {
  cases = [{ ...REPORTED_CASE }];
  stubBackend();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("CaseList", () => {
  it("lists every case by default", async () => {
    await loginAndReachCases();

    expect(await screen.findByText("Missing child")).toBeInTheDocument();
    expect(screen.getByText("REPORTED")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("refetches with the selected status filter", async () => {
    const user = await loginAndReachCases();
    await screen.findByText("Missing child");

    await user.selectOptions(screen.getByLabelText(/Status:/), "RESOLVED");

    await waitFor(() => {
      expect(screen.getByText("No cases match this filter.")).toBeInTheDocument();
    });
  });

  it("links each case to its detail screen", async () => {
    await loginAndReachCases();

    const link = await screen.findByRole("link", { name: REPORTED_CASE.case_id.slice(0, 8) });
    expect(link).toHaveAttribute("href", `/cases/${REPORTED_CASE.case_id}`);
  });
});
