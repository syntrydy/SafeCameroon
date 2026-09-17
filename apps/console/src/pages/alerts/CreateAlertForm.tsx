import { useState, type FormEvent } from "react";

import { createAlert, MISSING_CHILD_COMMUNITY_FIELDS, type Alert, type Severity } from "../../api/alerts";
import { ApiError } from "../../api/client";
import { useTranslation } from "../../i18n/LanguageContext";
import type { Translations } from "../../i18n/translations";

const SEVERITIES: Severity[] = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];

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

  return (
    <form onSubmit={handleSubmit} className="rounded border border-white/[0.08] p-4">
      <h3 className="mb-3 text-sm font-semibold text-white">{t.createAlertForm.heading}</h3>

      <label className="mb-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.severity}</span>
        <select
          value={severity}
          onChange={(event) => setSeverity(event.target.value as Severity)}
          className="w-full max-w-xs rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
        >
          {SEVERITIES.map((option) => (
            <option key={option} value={option} className="bg-slate-900 text-white">
              {option}
            </option>
          ))}
        </select>
      </label>

      <label className="mb-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.targetGeography}</span>
        <input
          required
          value={targetGeography}
          onChange={(event) => setTargetGeography(event.target.value)}
          placeholder={t.createAlertForm.targetGeographyPlaceholder}
          className="w-full max-w-xs rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
        />
      </label>

      {MISSING_CHILD_COMMUNITY_FIELDS.map((field) => (
        <label key={field} className="mb-3 block text-sm">
          <span className="mb-1 block font-medium text-slate-300">{labels[field]}</span>
          <input
            value={fieldValues[field] ?? ""}
            onChange={(event) => setFieldValues((current) => ({ ...current, [field]: event.target.value }))}
            className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
        </label>
      ))}

      {error && (
        <p role="alert" className="mb-3 text-sm text-red-300">
          {error}
        </p>
      )}

      <button
        type="submit"
        disabled={submitting}
        className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {submitting ? t.createAlertForm.creating : t.createAlertForm.createAlert}
      </button>
    </form>
  );
}
