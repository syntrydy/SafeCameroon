import { apiRequest } from "./client";
import type { Role } from "./organizations";

// Matches apps/api/src/auth.rs `LoginResponse`.
export interface LoginResponse {
  token: string;
  expires_in_seconds: number;
  reviewer_id: string;
  email: string;
  role: Role;
}

export function loginWithGoogle(idToken: string): Promise<LoginResponse> {
  return apiRequest<LoginResponse>("/v1/auth/google", {
    method: "POST",
    body: { id_token: idToken },
  });
}

export function logout(token: string): Promise<void> {
  return apiRequest<void>("/v1/auth/logout", { method: "POST", token });
}
