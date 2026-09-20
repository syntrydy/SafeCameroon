import { useEffect, useRef, useState, type FormEvent } from "react";

import { createAlert, MISSING_CHILD_COMMUNITY_FIELDS, type Alert, type AlertField, type Severity } from "../../api/alerts";
import type { ExtractedFields } from "../../api/extractions";
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

const fieldInputClassName =
  "w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20";

// Maps a report's AI-extracted fields onto this policy's alert fields.
// Deliberately excludes `contact_request` (the citizen reporter's own
// contact info -- never the alert's OFFICIAL_CONTACT, which would leak
// reporter PII into a public/community alert) and CASE_REFERENCE (an
// internal reviewer-chosen id the report text has no bearing on).
function mappedFieldValues(fields: ExtractedFields): Partial<Record<AlertField, string>> {
  const mapped: Partial<Record<AlertField, string>> = {};
  if (fields.age) mapped.APPROXIMATE_AGE = fields.age;
  if (fields.time) mapped.TIME_WINDOW = fields.time;
  if (fields.place) mapped.LAST_SEEN_GENERAL_AREA = fields.place;
  if (fields.incident_category) mapped.INCIDENT_CATEGORY = fields.incident_category;
  const description = [fields.person_description, fields.vehicle_details].filter(Boolean).join(" ");
  if (description) mapped.SAFE_DESCRIPTION = description;
  return mapped;
}

interface CreateAlertFormProps {
  token: string;
  caseId: string;
  onCreated: (alert: Alert) => void;
  // Set (with a fresh `appliedAt`) each time a reviewer applies one of the
  // linked report's AI extractions -- see ExtractionPanel's `onApply`. Only
  // fills fields the reviewer hasn't already typed into.
  suggestedFields?: { fields: ExtractedFields; appliedAt: number } | null;
}

export function CreateAlertForm({ token, caseId, onCreated, suggestedFields }: CreateAlertFormProps) {
  const { t } = useTranslation();
  const labels = fieldLabels(t);
  const [severity, setSeverity] = useState<Severity>("HIGH");
  const [targetGeography, setTargetGeography] = useState("");
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [suggestionsAppliedAt, setSuggestionsAppliedAt] = useState<number | null>(null);
  // Fields the reviewer has actually typed into, tracked independently of
  // their current value (which may still be "" if they typed then cleared
  // it). A suggestion can arrive at any point -- e.g. auto-applied the
  // moment the page loads, which can race a reviewer already typing -- so
  // "currently empty" alone isn't a safe signal that a field is still
  // untouched; this is checked instead of/alongside that.
  const touchedFieldsRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    if (!suggestedFields) return;
    const mapped = mappedFieldValues(suggestedFields.fields);
    setFieldValues((current) => {
      const next = { ...current };
      for (const [field, value] of Object.entries(mapped) as [AlertField, string][]) {
        if (!touchedFieldsRef.current.has(field) && !next[field]?.trim()) next[field] = value;
      }
      return next;
    });
    if (suggestedFields.fields.place && !touchedFieldsRef.current.has("targetGeography")) {
      setTargetGeography((current) => (current.trim() ? current : suggestedFields.fields.place!));
    }
    setSuggestionsAppliedAt(suggestedFields.appliedAt);
    // Re-runs only when a *new* apply happens (a fresh appliedAt), not on
    // every render -- `suggestedFields` itself is a fresh object each time
    // the parent re-renders, which would re-fire this on unrelated updates.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [suggestedFields?.appliedAt]);

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

      {suggestionsAppliedAt !== null && (
        <p role="status" className="mb-3 text-xs text-indigo-300">
          {t.createAlertForm.suggestionsApplied}
        </p>
      )}

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
            value={targetGeography}
            onChange={(event) => {
              touchedFieldsRef.current.add("targetGeography");
              setTargetGeography(event.target.value);
            }}
            placeholder={t.createAlertForm.targetGeographyPlaceholder}
            className={fieldInputClassName}
          />
        </label>

        {gridFields.map((field) => (
          <label key={field} className="block text-sm">
            <span className="mb-1 block font-medium text-slate-300">{labels[field]}</span>
            <input
              value={fieldValues[field] ?? ""}
              onChange={(event) => {
                touchedFieldsRef.current.add(field);
                setFieldValues((current) => ({ ...current, [field]: event.target.value }));
              }}
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
          onChange={(event) => {
            touchedFieldsRef.current.add("SAFE_DESCRIPTION");
            setFieldValues((current) => ({ ...current, SAFE_DESCRIPTION: event.target.value }));
          }}
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
