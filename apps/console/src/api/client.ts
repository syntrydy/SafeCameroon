/**
 * Thin fetch wrapper matching the backend's stable error shape
 * (docs/API.md section 8: `{ error: { code, message, request_id } }`).
 * No caching/retry layer — screens call this directly per the console's
 * "keep it simple" brief (prompts/10_ORG_CONSOLE.md).
 */

export class ApiError extends Error {
  readonly code: string;
  readonly requestId: string | null;
  readonly status: number;

  constructor(code: string, message: string, requestId: string | null, status: number) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.requestId = requestId;
    this.status = status;
  }
}

interface RequestOptions {
  method?: "GET" | "POST" | "PUT";
  body?: unknown;
  token?: string | null;
  headers?: Record<string, string>;
}

interface BackendErrorBody {
  error: {
    code: string;
    message: string;
    request_id: string;
  };
}

function isBackendErrorBody(value: unknown): value is BackendErrorBody {
  if (typeof value !== "object" || value === null || !("error" in value)) {
    return false;
  }
  const error = (value as { error: unknown }).error;
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error
  );
}

function safeParseJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

export async function apiRequest<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "";
  const headers: Record<string, string> = { "Content-Type": "application/json", ...options.headers };
  if (options.token) {
    headers.Authorization = `Bearer ${options.token}`;
  }

  let response: Response;
  try {
    response = await fetch(`${baseUrl}${path}`, {
      method: options.method ?? "GET",
      headers,
      body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
    });
  } catch {
    throw new ApiError(
      "NETWORK_ERROR",
      "Could not reach the server. Check your connection and try again.",
      null,
      0,
    );
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const text = await response.text();
  const data = text ? safeParseJson(text) : null;

  if (!response.ok) {
    if (isBackendErrorBody(data)) {
      throw new ApiError(data.error.code, data.error.message, data.error.request_id, response.status);
    }
    throw new ApiError(
      "UNEXPECTED_RESPONSE",
      "An unexpected error occurred.",
      null,
      response.status,
    );
  }

  return data as T;
}
