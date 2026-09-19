import { useState, type FormEvent } from "react";

import { createAlert, MISSING_CHILD_COMMUNITY_FIELDS, type Alert, type AlertField, type Severity } from "../../api/alerts";
import { ApiError } from "../../api/client";
import { useTranslation } from "../../i18n/LanguageContext";
import type { Translations } from "../../i18n/translations";

const SEVERITIES: Severity[] = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];

const GEOGRAPHY_SUGGESTIONS_ID = "create-alert-geography-suggestions";
const INCIDENT_CATEGORY_SUGGESTIONS_ID = "create-alert-incident-category-suggestions";
const TIME_WINDOW_SUGGESTIONS_ID = "create-alert-time-window-suggestions";

// Fields with a small, well-known set of common values get a <datalist> of
// suggestions -- still free text underneath (the backend stores every field
// as a plain string, docs/DOMAIN_MODEL.md), just faster to fill for the
// common case than typing from scratch.
const FIELD_SUGGESTIONS_ID: Partial<Record<AlertField, string>> = {
  LAST_SEEN_GENERAL_AREA: GEOGRAPHY_SUGGESTIONS_ID,
  INCIDENT_CATEGORY: INCIDENT_CATEGORY_SUGGESTIONS_ID,
  TIME_WINDOW: TIME_WINDOW_SUGGESTIONS_ID,
};

function fieldLabels(t: Translations): Record<string, string> {
  return {
    INCIDENT_CATEGORY: t.createAlertForm.fieldIncidentCategory,
    APPROXIMATE_AGE: t.createAlertForm.fieldApproximateAge,
    LAST_SEEN_GENERAL_AREA: t.createAlertForm.fieldLastSeenArea,
    TIME_WINDOW: t.createAlertForm.fieldTimeWindow,
    SAFE_DESCRIPTION: t.createAlertForm.fieldSafeDescription,
    OFFICIAL_CONTACT: t.createAlertForm.fieldOfficialContact,
    CASE_REFERENCE: t.createAlertForm.fieldCaseReference,
  };
}

const fieldInputClassName =
  "w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20";

interface CreateAlertFormProps {
  token: string;
  caseId: string;
  onCreated: (alert: Alert) => void;
}

export function CreateAlertForm({ token, caseId, onCreated }: CreateAlertFormProps) {
  const { t } = useTranslation();
  const labels = fieldLabels(t);
  const [severity, setSeverity] = useState<Severity>("HIGH");
  const [targetGeography, setTargetGeography] = useState("");
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const alert = await createAlert(token, caseId, {
        severity,
        targetGeography,
        fields: MISSING_CHILD_COMMUNITY_FIELDS.filter((field) => fieldValues[field]?.trim()).map(
          (field) => ({ field, value: fieldValues[field].trim() }),
        ),
      });
      onCreated(alert);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setSubmitting(false);
    }
  }

  const gridFields = MISSING_CHILD_COMMUNITY_FIELDS.filter((field) => field !== "SAFE_DESCRIPTION");

  return (
    <form onSubmit={handleSubmit} className="rounded border border-white/[0.08] p-4">
      <h2 className="mb-3 text-sm font-semibold text-white">{t.createAlertForm.heading}</h2>

      <datalist id={GEOGRAPHY_SUGGESTIONS_ID}>
        {t.createAlertForm.geographySuggestions.map((suggestion) => (
          <option key={suggestion} value={suggestion} />
        ))}
      </datalist>
      <datalist id={INCIDENT_CATEGORY_SUGGESTIONS_ID}>
        {t.createAlertForm.incidentCategorySuggestions.map((suggestion) => (
          <option key={suggestion} value={suggestion} />
        ))}
      </datalist>
      <datalist id={TIME_WINDOW_SUGGESTIONS_ID}>
        {t.createAlertForm.timeWindowSuggestions.map((suggestion) => (
          <option key={suggestion} value={suggestion} />
        ))}
      </datalist>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.severity}</span>
          <select
            value={severity}
            onChange={(event) => setSeverity(event.target.value as Severity)}
            className={fieldInputClassName}
          >
            {SEVERITIES.map((option) => (
              <option key={option} value={option} className="bg-slate-900 text-white">
                {option}
              </option>
            ))}
          </select>
        </label>

        <label className="block text-sm">
          <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.targetGeography}</span>
          <input
            required
            list={GEOGRAPHY_SUGGESTIONS_ID}
            value={targetGeography}
            onChange={(event) => setTargetGeography(event.target.value)}
            placeholder={t.createAlertForm.targetGeographyPlaceholder}
            className={fieldInputClassName}
          />
        </label>

        {gridFields.map((field) => (
          <label key={field} className="block text-sm">
            <span className="mb-1 block font-medium text-slate-300">{labels[field]}</span>
            <input
              list={FIELD_SUGGESTIONS_ID[field]}
              value={fieldValues[field] ?? ""}
              onChange={(event) => setFieldValues((current) => ({ ...current, [field]: event.target.value }))}
              className={fieldInputClassName}
            />
          </label>
        ))}
      </div>

      <label className="mt-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.fieldSafeDescription}</span>
        <textarea
          rows={3}
          value={fieldValues.SAFE_DESCRIPTION ?? ""}
          onChange={(event) =>
            setFieldValues((current) => ({ ...current, SAFE_DESCRIPTION: event.target.value }))
          }
          className={fieldInputClassName}
        />
      </label>

      {error && (
        <p role="alert" className="mt-3 text-sm text-red-300">
          {error}
        </p>
      )}

      <button
        type="submit"
        disabled={submitting}
        className="mt-3 rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {submitting ? t.createAlertForm.creating : t.createAlertForm.createAlert}
      </button>
    </form>
  );
}
