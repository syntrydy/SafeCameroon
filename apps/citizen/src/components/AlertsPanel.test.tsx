import { render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AlertsPanel } from "./AlertsPanel";
import { LanguageProvider } from "../i18n/LanguageContext";

function renderPanel() {
  return render(
    <LanguageProvider>
      <AlertsPanel />
    </LanguageProvider>,
  );
}

vi.mock("../push/subscribe", () => ({
  subscribeToPush: vi.fn().mockResolvedValue({
    endpoint: "https://push.example/device",
    keys: { p256dh: "key", auth: "secret" },
  }),
  unsubscribeFromPush: vi.fn().mockResolvedValue(undefined),
  PushUnsupportedError: class PushUnsupportedError extends Error {},
  PushPermissionDeniedError: class PushPermissionDeniedError extends Error {},
}));

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("AlertsPanel", () => {
  it("subscribes to alerts and remembers the subscription for this device", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(201, {
          consumer_id: "11111111-1111-1111-1111-111111111111",
          subscription_id: "22222222-2222-2222-2222-222222222222",
          management_token: "a-fake-management-token",
        }),
      ),
    );
    const user = userEvent.setup();
    renderPanel();

    await screen.findByText("Get missing-child alerts");
    await user.type(screen.getByLabelText("Area"), "Douala");
    await user.click(screen.getByRole("button", { name: "Enable alerts" }));

    await waitFor(() => {
      expect(screen.getByText("Alerts are on for this device.")).toBeInTheDocument();
    });
    expect(JSON.parse(window.localStorage.getItem("safecameroon-citizen-alert-subscription")!)).toEqual({
      subscriptionId: "22222222-2222-2222-2222-222222222222",
      managementToken: "a-fake-management-token",
    });
  });

  it("subscribes to multiple alert types and areas in one subscription", async () => {
    let sentBody: { incident_types: string[]; geography: string[] } | null = null;
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((_input: string, init?: RequestInit) => {
        sentBody = JSON.parse(init?.body as string);
        return Promise.resolve(
          jsonResponse(201, {
            consumer_id: "11111111-1111-1111-1111-111111111111",
            subscription_id: "22222222-2222-2222-2222-222222222222",
            management_token: "a-fake-management-token",
          }),
        );
      }),
    );
    const user = userEvent.setup();
    renderPanel();

    await screen.findByText("Get missing-child alerts");
    await user.click(screen.getByRole("combobox", { name: "Alert type" }));
    await user.click(await screen.findByRole("option", { name: "Other protection incident" }));
    await user.type(screen.getByLabelText("Area"), "Douala");
    await user.keyboard("{Enter}");
    await user.type(screen.getByLabelText("Area"), "Yaounde");
    await user.keyboard("{Enter}");
    await user.click(screen.getByRole("button", { name: "Enable alerts" }));

    await waitFor(() => {
      expect(screen.getByText("Alerts are on for this device.")).toBeInTheDocument();
    });
    expect(sentBody).toEqual(
      expect.objectContaining({
        incident_types: ["MISSING_CHILD", "OTHER_PROTECTION_INCIDENT"],
        geography: ["Douala", "Yaounde"],
      }),
    );
  });

  it("loads an existing subscription from this device and shows it as on", async () => {
    window.localStorage.setItem(
      "safecameroon-citizen-alert-subscription",
      JSON.stringify({ subscriptionId: "sub-1", managementToken: "token-1" }),
    );
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(200, {
          incident_types: ["MISSING_CHILD"],
          minimum_severity: "HIGH",
          geography: ["Douala"],
        }),
      ),
    );

    renderPanel();

    await waitFor(() => {
      expect(screen.getByText("Alerts are on for this device.")).toBeInTheDocument();
    });
    expect(screen.getByText("Douala")).toBeInTheDocument();
  });

  it("clears a subscription this device can no longer authenticate", async () => {
    window.localStorage.setItem(
      "safecameroon-citizen-alert-subscription",
      JSON.stringify({ subscriptionId: "sub-1", managementToken: "stale-token" }),
    );
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(404, {
          error: { code: "CITIZEN_SUBSCRIPTION_NOT_FOUND", message: "Not found.", request_id: null },
        }),
      ),
    );

    renderPanel();

    await screen.findByText("Get missing-child alerts");
    expect(window.localStorage.getItem("safecameroon-citizen-alert-subscription")).toBeNull();
  });

  it("turns off alerts and returns to the signup form on request", async () => {
    window.localStorage.setItem(
      "safecameroon-citizen-alert-subscription",
      JSON.stringify({ subscriptionId: "sub-1", managementToken: "token-1" }),
    );
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((_input: string, init?: RequestInit) => {
        if ((init?.method ?? "GET") === "GET") {
          return Promise.resolve(
            jsonResponse(200, {
              incident_types: ["MISSING_CHILD"],
              minimum_severity: "HIGH",
              geography: ["Douala"],
            }),
          );
        }
        return Promise.resolve(new Response(null, { status: 204 }));
      }),
    );
    const user = userEvent.setup();
    renderPanel();

    await user.click(await screen.findByRole("button", { name: "Turn off alerts" }));

    await waitFor(() => {
      expect(screen.getByText("Alerts turned off")).toBeInTheDocument();
    });
    expect(window.localStorage.getItem("safecameroon-citizen-alert-subscription")).toBeNull();
  });
});
