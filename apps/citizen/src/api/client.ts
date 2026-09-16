interface ApiErrorPayload {
  error: { code: string; message: string; request_id: string | null };
}

// Carries the HTTP status alongside the backend's own error envelope, so
// callers can tell a validation failure (4xx, resubmitting the same data
// will never work) from a transient one (5xx/429, worth queuing for retry)
// without re-deriving that decision from the code string.
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly requestId: string | null;

  constructor(status: number, code: string, message: string, requestId: string | null) {
    super(message);
    this.status = status;
    this.code = code;
    this.requestId = requestId;
  }
}

const BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "";

interface RequestOptions extends RequestInit {
  idempotencyKey?: string;
}

export async function apiRequest<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const { idempotencyKey, headers: initHeaders, ...init } = options;
  const headers = new Headers(initHeaders);
  if (init.body !== undefined && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  if (idempotencyKey) {
    headers.set("Idempotency-Key", idempotencyKey);
  }

  // A network failure (offline, DNS, CORS) throws a TypeError here rather
  // than resolving -- callers rely on that to decide whether to queue.
  const response = await fetch(`${BASE_URL}${path}`, { ...init, headers });

  if (response.status === 204) {
    return undefined as T;
  }

  const body = await response.json().catch(() => null);

  if (!response.ok) {
    const payload = body as ApiErrorPayload | null;
    throw new ApiError(
      response.status,
      payload?.error?.code ?? "UNKNOWN_ERROR",
      payload?.error?.message ?? "An unexpected error occurred.",
      payload?.error?.request_id ?? null,
    );
  }

  return body as T;
}

/** A 4xx response (other than 429) means the submitted data itself is
 * invalid -- retrying unchanged will never succeed, so it should surface to
 * the user instead of being queued. A 429 is still transient: the same
 * request will succeed once the rate-limit window rolls over. Anything else
 * (network failure, 5xx) is also transient. */
export function isPermanentFailure(error: unknown): boolean {
  return (
    error instanceof ApiError && error.status >= 400 && error.status < 500 && error.status !== 429
  );
}
