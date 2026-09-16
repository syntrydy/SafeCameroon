import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ExtractionPanel } from "./ExtractionPanel";

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ExtractionPanel", () => {
  it("starts collapsed and shows no extractions yet after opening", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(jsonResponse(200, [])));
    const user = userEvent.setup();
    render(<ExtractionPanel token="token-1" reportId="report-1" />);

    expect(screen.queryByText("No extraction requested yet.")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "AI extraction" }));

    await waitFor(() => {
      expect(screen.getByText("No extraction requested yet.")).toBeInTheDocument();
    });
  });

  it("requests an extraction and shows the suggested fields", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((_input: string, init?: RequestInit) => {
        if ((init?.method ?? "GET") === "GET") {
          return Promise.resolve(jsonResponse(200, []));
        }
        return Promise.resolve(
          jsonResponse(201, {
            report_id: "report-1",
            requested_by: "reviewer-1",
            provider: "OPENROUTER",
            model: "openai/gpt-4o-mini",
            prompt_version: "v1",
            fields: {
              person_description: "a young girl in a blue uniform",
              age: "about 8 years old",
              time: null,
              place: "Douala",
              incident_category: null,
              vehicle_details: null,
              contact_request: null,
            },
          }),
        );
      }),
    );
    const user = userEvent.setup();
    render(<ExtractionPanel token="token-1" reportId="report-1" />);

    await user.click(screen.getByRole("button", { name: "AI extraction" }));
    await user.click(await screen.findByRole("button", { name: "Extract candidate info" }));

    await waitFor(() => {
      expect(screen.getByText("a young girl in a blue uniform")).toBeInTheDocument();
    });
    expect(screen.getByText("Douala")).toBeInTheDocument();
    expect(screen.getByText("openrouter/openai/gpt-4o-mini")).toBeInTheDocument();
  });

  it("shows the backend's error message when extraction fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((_input: string, init?: RequestInit) => {
        if ((init?.method ?? "GET") === "GET") {
          return Promise.resolve(jsonResponse(200, []));
        }
        return Promise.resolve(
          jsonResponse(502, {
            error: {
              code: "EXTRACTION_FAILED",
              message: "The extraction could not be completed. Please try again.",
              request_id: "req-1",
            },
          }),
        );
      }),
    );
    const user = userEvent.setup();
    render(<ExtractionPanel token="token-1" reportId="report-1" />);

    await user.click(screen.getByRole("button", { name: "AI extraction" }));
    await user.click(await screen.findByRole("button", { name: "Extract candidate info" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The extraction could not be completed. Please try again.",
    );
  });
});
