import { useState, type FormEvent } from "react";

import { createAlert, MISSING_CHILD_COMMUNITY_FIELDS, type Alert, type Severity } from "../../api/alerts";
import { ApiError } from "../../api/client";

const SEVERITIES: Severity[] = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];

const FIELD_LABELS: Record<string, string> = {
  INCIDENT_CATEGORY: "Incident category",
  APPROXIMATE_AGE: "Approximate age",
  LAST_SEEN_GENERAL_AREA: "Last seen (general area)",
  TIME_WINDOW: "Time window",
  SAFE_DESCRIPTION: "Safe description",
  OFFICIAL_CONTACT: "Official contact",
  CASE_REFERENCE: "Case reference",
};

interface CreateAlertFormProps {
  token: string;
  caseId: string;
  onCreated: (alert: Alert) => void;
}

export function CreateAlertForm({ token, caseId, onCreated }: CreateAlertFormProps) {
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
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="rounded border border-slate-200 p-4">
      <h3 className="mb-3 text-sm font-semibold text-slate-900">Create community alert</h3>

      <label className="mb-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-700">Severity</span>
        <select
          value={severity}
          onChange={(event) => setSeverity(event.target.value as Severity)}
          className="w-full max-w-xs rounded border border-slate-300 px-2 py-1 text-sm"
        >
          {SEVERITIES.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </label>

      <label className="mb-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-700">Target geography</span>
        <input
          required
          value={targetGeography}
          onChange={(event) => setTargetGeography(event.target.value)}
          placeholder="e.g. Douala, Bonamoussadi"
          className="w-full max-w-xs rounded border border-slate-300 px-2 py-1 text-sm"
        />
      </label>

      {MISSING_CHILD_COMMUNITY_FIELDS.map((field) => (
        <label key={field} className="mb-3 block text-sm">
          <span className="mb-1 block font-medium text-slate-700">{FIELD_LABELS[field]}</span>
          <input
            value={fieldValues[field] ?? ""}
            onChange={(event) => setFieldValues((current) => ({ ...current, [field]: event.target.value }))}
            className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
          />
        </label>
      ))}

      {error && (
        <p role="alert" className="mb-3 text-sm text-red-700">
          {error}
        </p>
      )}

      <button
        type="submit"
        disabled={submitting}
        className="rounded bg-slate-900 px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
      >
        {submitting ? "Creating..." : "Create alert"}
      </button>
    </form>
  );
}
