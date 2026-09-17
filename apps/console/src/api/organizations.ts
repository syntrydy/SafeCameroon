import type { IncidentType } from "./cases";
import { apiRequest } from "./client";
import type { AlertVisibility } from "./alerts";

// Matches crates/domain/src/organization.rs `Role`.
export type Role = "PLATFORM_ADMIN" | "ORG_ADMIN" | "MEMBER";

// Matches apps/api/src/organizations.rs `OrganizationResponse`.
export interface Organization {
  organization_id: string;
  name: string;
  verified_incident_types: IncidentType[];
  verified_alert_visibilities: AlertVisibility[];
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

export function createOrganization(token: string, name: string): Promise<Organization> {
  return apiRequest<Organization>("/v1/organizations", {
    method: "POST",
    token,
    body: { name },
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
