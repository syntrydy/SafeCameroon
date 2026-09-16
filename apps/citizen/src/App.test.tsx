import { render, screen, waitFor } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./app";

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function resetDatabase(): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.deleteDatabase("safecameroon-citizen");
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error as Error);
  });
}

beforeEach(async () => {
  window.localStorage.clear();
  await resetDatabase();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("citizen reporting app", () => {
  it("shows a validation error instead of submitting blank content", async () => {
    vi.stubGlobal("fetch", vi.fn());
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Send report" }));

    expect(await screen.findByText("Please describe the situation.")).toBeInTheDocument();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("submits a report and shows the reference code", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(201, {
          report_id: "11111111-1111-1111-1111-111111111111",
          reference_code: "SC-abc123",
          status: "RECEIVED",
        }),
      ),
    );
    const user = userEvent.setup();
    render(<App />);

    await user.type(
      screen.getByLabelText("What is happening?"),
      "A child is missing near the market.",
    );
    await user.click(screen.getByRole("button", { name: "Send report" }));

    expect(await screen.findByText("SC-abc123")).toBeInTheDocument();
    expect(screen.getByText("Report received")).toBeInTheDocument();
  });

  it("saves the report locally and reassures the user when the network fails", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));
    const user = userEvent.setup();
    render(<App />);

    await user.type(
      screen.getByLabelText("What is happening?"),
      "A child is missing near the market.",
    );
    await user.click(screen.getByRole("button", { name: "Send report" }));

    await waitFor(() => {
      expect(screen.getByText("Saved on this device")).toBeInTheDocument();
    });
  });
});
