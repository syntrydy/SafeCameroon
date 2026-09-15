import { apiRequest } from "./client";

// Matches apps/api/src/auth.rs `LoginResponse`.
export interface LoginResponse {
  token: string;
  expires_in_seconds: number;
  reviewer_id: string;
}

export function login(email: string, password: string): Promise<LoginResponse> {
  return apiRequest<LoginResponse>("/v1/auth/login", {
    method: "POST",
    body: { email, password },
  });
}

export function logout(token: string): Promise<void> {
  return apiRequest<void>("/v1/auth/logout", { method: "POST", token });
}
