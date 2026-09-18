import { render, screen } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { beforeAll, describe, expect, it, vi } from "vitest";

import { ReportForm } from "./ReportForm";
import { LanguageProvider } from "../i18n/LanguageContext";

// jsdom doesn't implement these -- ReportForm calls them to preview a
// selected photo, which isn't itself under test here.
beforeAll(() => {
  URL.createObjectURL = vi.fn().mockReturnValue("blob:fake-preview-url");
  URL.revokeObjectURL = vi.fn();
});

function renderForm(onSubmit = vi.fn()) {
  return {
    onSubmit,
    ...render(
      <LanguageProvider>
        <ReportForm submitting={false} errorMessage={null} onSubmit={onSubmit} />
      </LanguageProvider>,
    ),
  };
}

function fileInput(container: Element): HTMLInputElement {
  return container.querySelector('input[type="file"]') as HTMLInputElement;
}

describe("ReportForm photo upload", () => {
  it("lets the browser offer both camera and photo library (no capture attribute)", () => {
    const { container } = renderForm();

    expect(fileInput(container).hasAttribute("capture")).toBe(false);
  });

  it("accepts exactly the three most common image formats", () => {
    const { container } = renderForm();

    expect(fileInput(container).accept).toBe("image/jpeg,image/png,image/webp");
  });

  it("rejects a photo over 2MB with a clear error", async () => {
    const { container } = renderForm();
    const user = userEvent.setup();

    const oversized = new File([new Uint8Array(2 * 1024 * 1024 + 1)], "big.jpg", {
      type: "image/jpeg",
    });
    await user.upload(fileInput(container), oversized);

    expect(await screen.findByText("Please choose a photo under 2MB.")).toBeInTheDocument();
    expect(screen.queryByAltText("Selected")).not.toBeInTheDocument();
  });

  it("accepts a photo at or under 2MB", async () => {
    const { container } = renderForm();
    const user = userEvent.setup();

    const withinLimit = new File([new Uint8Array(2 * 1024 * 1024)], "ok.jpg", {
      type: "image/jpeg",
    });
    await user.upload(fileInput(container), withinLimit);

    expect(await screen.findByAltText("Selected")).toBeInTheDocument();
    expect(screen.queryByText("Please choose a photo under 2MB.")).not.toBeInTheDocument();
  });
});
