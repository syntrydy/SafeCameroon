import { apiRequest } from "./client";

// Matches apps/api/src/extractions.rs `ExtractionResponse`/`ExtractedFieldsResponse`.
// Every field is an unverified AI suggestion (docs/AI.md) -- never treated
// as confirmed, and never applied to a report or case automatically.
export interface ExtractedFields {
  person_description: string | null;
  age: string | null;
  time: string | null;
  place: string | null;
  incident_category: string | null;
  vehicle_details: string | null;
  contact_request: string | null;
}

export interface Extraction {
  report_id: string;
  requested_by: string;
  provider: string;
  model: string;
  prompt_version: string;
  fields: ExtractedFields;
}

export function createExtraction(token: string, reportId: string): Promise<Extraction> {
  return apiRequest<Extraction>(`/v1/reports/${reportId}/extractions`, {
    method: "POST",
    token,
  });
}

export function listExtractions(token: string, reportId: string): Promise<Extraction[]> {
  return apiRequest<Extraction[]>(`/v1/reports/${reportId}/extractions`, { token });
}
