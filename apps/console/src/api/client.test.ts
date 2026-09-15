import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError, apiRequest } from "./client";

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

async function captureApiError(promise: Promise<unknown>): Promise<ApiError> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ApiError) {
      return error;
    }
    throw error;
  }
  throw new Error("expected the promise to reject with an ApiError");
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("apiRequest", () => {
  it("returns the parsed body on success", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(jsonResponse(200, { ok: true })));

    await expect(apiRequest<{ ok: boolean }>("/v1/health")).resolves.toEqual({ ok: true });
  });

  it("returns undefined for a 204 response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 204 })));

    await expect(apiRequest("/v1/auth/logout", { method: "POST" })).resolves.toBeUndefined();
  });

  it("throws an ApiError carrying the backend's code/message/request_id", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        jsonResponse(401, {
          error: {
            code: "INVALID_CREDENTIALS",
            message: "Invalid email or password.",
            request_id: "11111111-1111-1111-1111-111111111111",
          },
        }),
      ),
    );

    const error = await captureApiError(apiRequest("/v1/auth/login", { method: "POST" }));

    expect(error.code).toBe("INVALID_CREDENTIALS");
    expect(error.message).toBe("Invalid email or password.");
    expect(error.requestId).toBe("11111111-1111-1111-1111-111111111111");
    expect(error.status).toBe(401);
  });

  it("throws a NETWORK_ERROR ApiError when fetch itself fails", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("network down")));

    const error = await captureApiError(apiRequest("/v1/health"));

    expect(error.code).toBe("NETWORK_ERROR");
  });

  it("falls back to UNEXPECTED_RESPONSE when an error body doesn't match the backend shape", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("<html>502</html>", { status: 502 })));

    const error = await captureApiError(apiRequest("/v1/health"));

    expect(error.code).toBe("UNEXPECTED_RESPONSE");
    expect(error.status).toBe(502);
  });

  it("attaches an Authorization header when a token is provided", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, {}));
    vi.stubGlobal("fetch", fetchMock);

    await apiRequest("/v1/cases", { token: "session-token" });

    const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect((init.headers as Record<string, string>).Authorization).toBe("Bearer session-token");
  });
});
