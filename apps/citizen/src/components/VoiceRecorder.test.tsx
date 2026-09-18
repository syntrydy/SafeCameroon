import { render, screen } from "@testing-library/preact";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { VoiceRecorder } from "./VoiceRecorder";
import { LanguageProvider } from "../i18n/LanguageContext";

function renderRecorder(onConfirm: (transcript: string) => void) {
  return render(
    <LanguageProvider>
      <VoiceRecorder onConfirm={onConfirm} />
    </LanguageProvider>,
  );
}

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

/** Stands in for the browser's real `MediaRecorder` -- `start()` fires one
 * data chunk immediately, `stop()` fires `onstop` synchronously, close
 * enough to drive the component through record -> stop -> transcribe. */
class FakeMediaRecorder {
  static isTypeSupported = vi.fn().mockReturnValue(true);
  ondataavailable: ((event: { data: Blob }) => void) | null = null;
  onstop: (() => void) | null = null;
  mimeType = "audio/ogg;codecs=opus";

  constructor(public stream: MediaStream) {}

  start() {
    this.ondataavailable?.({ data: new Blob(["fake-audio"], { type: this.mimeType }) });
  }

  stop() {
    this.onstop?.();
  }
}

function fakeMediaStream(): MediaStream {
  return { getTracks: () => [{ stop: vi.fn() }] } as unknown as MediaStream;
}

beforeEach(() => {
  vi.stubGlobal("MediaRecorder", FakeMediaRecorder);
  Object.defineProperty(navigator, "mediaDevices", {
    value: { getUserMedia: vi.fn().mockResolvedValue(fakeMediaStream()) },
    configurable: true,
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("VoiceRecorder", () => {
  it("records, transcribes, and confirms the (edited) transcript", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(jsonResponse(200, { transcript: "My daughter is missing." })),
    );
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    renderRecorder(onConfirm);

    await user.click(screen.getByRole("button", { name: "Record" }));
    await user.click(await screen.findByRole("button", { name: "Stop" }));

    const textarea = await screen.findByDisplayValue("My daughter is missing.");
    await user.clear(textarea);
    await user.type(textarea, "Edited transcript.");
    await user.click(screen.getByRole("button", { name: "Use this text" }));

    expect(onConfirm).toHaveBeenCalledWith("Edited transcript.");
  });

  it("lets the reporter re-record instead of confirming", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(jsonResponse(200, { transcript: "First take." })),
    );
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    renderRecorder(onConfirm);

    await user.click(screen.getByRole("button", { name: "Record" }));
    await user.click(await screen.findByRole("button", { name: "Stop" }));
    await screen.findByDisplayValue("First take.");

    await user.click(screen.getByRole("button", { name: "Record again" }));

    expect(await screen.findByRole("button", { name: "Record" })).toBeInTheDocument();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("shows a permission error when the microphone is denied", async () => {
    Object.defineProperty(navigator, "mediaDevices", {
      value: { getUserMedia: vi.fn().mockRejectedValue(new Error("denied")) },
      configurable: true,
    });
    const user = userEvent.setup();
    renderRecorder(vi.fn());

    await user.click(screen.getByRole("button", { name: "Record" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Microphone access was denied");
  });

  it("shows a transcription-failed error when the request fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(502, {
          error: { code: "TRANSCRIPTION_FAILED", message: "Failed.", request_id: null },
        }),
      ),
    );
    const user = userEvent.setup();
    renderRecorder(vi.fn());

    await user.click(screen.getByRole("button", { name: "Record" }));
    await user.click(await screen.findByRole("button", { name: "Stop" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("couldn't transcribe");
  });
});
