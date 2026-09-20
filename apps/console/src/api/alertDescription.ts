import { apiRequest } from "./client";

// Matches apps/api/src/alert_description.rs `AlertDescriptionSuggestionResponse`.
// An unverified AI suggestion (docs/AI.md) -- never applied to an alert
// automatically; a reviewer reads it, then decides whether to use it.
export interface AlertDescriptionSuggestion {
  description_en: string;
  description_fr: string;
  provider: string;
  model: string;
  prompt_version: string;
}

export function generateAlertDescription(
  token: string,
  caseId: string,
  sourceText: string,
): Promise<AlertDescriptionSuggestion> {
  return apiRequest<AlertDescriptionSuggestion>(`/v1/cases/${caseId}/alert-description-suggestions`, {
    method: "POST",
    token,
    body: { source_text: sourceText },
  });
}
