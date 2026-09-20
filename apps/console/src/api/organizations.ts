import type { IncidentType } from "./cases";
import { apiRequest } from "./client";
import type { AlertVisibility } from "./alerts";

// Matches crates/domain/src/organization.rs `Role`.
export type Role = "PLATFORM_ADMIN" | "ORG_ADMIN" | "MEMBER";

// Matches apps/api/src/organizations.rs `OrganizationResponse`.
export interface Organization {
  organization_id: string;
  name: string;
  description: string | null;
  location: string | null;
  contact?: string | null;
  verified_incident_types: IncidentType[];
  verified_alert_visibilities: AlertVisibility[];
  // The consumer this organization's alert subscription and delivery
  // preference live under (Organization::consumer_id). Null only for
  // organizations that predate this link.
  consumer_id: string | null;
  is_active: boolean;
}

// Matches apps/api/src/organizations.rs `MemberResponse`.
export interface Member {
  reviewer_id: string;
  email: string;
  role: Role;
}

export function listOrganizations(token: string): Promise<Organization[]> {
  return apiRequest<Organization[]>("/v1/organizations", { token });
}

export function getOrganization(token: string, organizationId: string): Promise<Organization> {
  return apiRequest<Organization>(`/v1/organizations/${organizationId}`, { token });
}

export function createOrganization(
  token: string,
  name: string,
  description?: string,
  location?: string,
): Promise<Organization> {
  return apiRequest<Organization>("/v1/organizations", {
    method: "POST",
    token,
    body: { name, description, location },
  });
}

export function updateOrganizationProfile(
  token: string,
  organizationId: string,
  description: string,
  location: string,
  contact?: string,
): Promise<Organization> {
  return apiRequest<Organization>(`/v1/organizations/${organizationId}/profile`, {
    method: "PUT",
    token,
    body: { description, location, contact },
  });
}

export function deactivateOrganization(token: string, organizationId: string): Promise<Organization> {
  return apiRequest<Organization>(`/v1/organizations/${organizationId}/deactivate`, {
    method: "POST",
    token,
  });
}

export function reactivateOrganization(token: string, organizationId: string): Promise<Organization> {
  return apiRequest<Organization>(`/v1/organizations/${organizationId}/reactivate`, {
    method: "POST",
    token,
  });
}

export function setTrustGrants(
  token: string,
  organizationId: string,
  verifiedIncidentTypes: IncidentType[],
  verifiedAlertVisibilities: AlertVisibility[],
): Promise<Organization> {
  return apiRequest<Organization>(`/v1/organizations/${organizationId}/trust`, {
    method: "PUT",
    token,
    body: {
      verified_incident_types: verifiedIncidentTypes,
      verified_alert_visibilities: verifiedAlertVisibilities,
    },
  });
}

export function listMembers(token: string, organizationId: string): Promise<Member[]> {
  return apiRequest<Member[]>(`/v1/organizations/${organizationId}/members`, { token });
}
