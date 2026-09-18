import { apiRequest } from "./client";
import type { Role } from "./organizations";

// Matches apps/api/src/auth.rs `LoginResponse`.
export interface LoginResponse {
  token: string;
  expires_in_seconds: number;
  reviewer_id: string;
  email: string;
  role: Role;
  organization_id: string | null;
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

// Matches apps/api/src/auth.rs `RegisterResponse`. Used here for an org
// admin inviting a MEMBER into their own organization
// (authorize_membership_grant already restricts what `token`'s caller may
// register — the server, not this client, is the real boundary).
export interface RegisterResponse {
  reviewer_id: string;
  email: string;
  role: Role;
  organization_id: string | null;
}

export function registerMember(
  token: string,
  email: string,
  organizationId: string,
): Promise<RegisterResponse> {
  return apiRequest<RegisterResponse>("/v1/auth/register", {
    method: "POST",
    token,
    body: { email, role: "MEMBER", organization_id: organizationId },
  });
}

// A platform admin registering the first (or an additional) ORG_ADMIN for a
// given organization — same endpoint, different role, only ever callable by
// a PLATFORM_ADMIN token (authorize_membership_grant enforces this
// server-side regardless of what this client sends).
export function registerOrgAdmin(
  token: string,
  email: string,
  organizationId: string,
): Promise<RegisterResponse> {
  return apiRequest<RegisterResponse>("/v1/auth/register", {
    method: "POST",
    token,
    body: { email, role: "ORG_ADMIN", organization_id: organizationId },
  });
}
