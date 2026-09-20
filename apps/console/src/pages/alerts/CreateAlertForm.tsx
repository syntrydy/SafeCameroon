import { useEffect, useRef, useState, type FormEvent } from "react";

import { createAlert, type Alert, type Severity } from "../../api/alerts";
import type { ExtractedFields } from "../../api/extractions";
import { ApiError } from "../../api/client";
import { useTranslation } from "../../i18n/LanguageContext";

const SEVERITIES: Severity[] = ["LOW", "MEDIUM", "HIGH", "CRITICAL"];

const fieldInputClassName =
  "w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20";

// Drafts a starting alert description from the report's AI extraction --
// a mechanical join of whatever fragments were found, not fluent prose, so
// the reviewer is expected to read and edit it before sending. Deliberately
// excludes `contact_request` (the citizen reporter's own contact info --
// must never leak into a public/community alert).
function composeAlertDescription(fields: ExtractedFields): string {
  return [
    fields.person_description && `${fields.person_description}.`,
    fields.age && `Age: ${fields.age}.`,
    fields.time && `Time: ${fields.time}.`,
    fields.place && `Location: ${fields.place}.`,
    fields.incident_category && `Category: ${fields.incident_category}.`,
    fields.vehicle_details && `Vehicle: ${fields.vehicle_details}.`,
  ]
    .filter(Boolean)
    .join(" ");
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
  const [severity, setSeverity] = useState<Severity>("HIGH");
  const [targetGeography, setTargetGeography] = useState("");
  const [description, setDescription] = useState("");
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
    if (!touchedFieldsRef.current.has("description")) {
      const composed = composeAlertDescription(suggestedFields.fields);
      if (composed) {
        setDescription((current) => (current.trim() ? current : composed));
      }
    }
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
        fields: description.trim() ? [{ field: "SAFE_DESCRIPTION", value: description.trim() }] : [],
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
      </div>

      <label className="mt-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.description}</span>
        <textarea
          rows={5}
          value={description}
          onChange={(event) => {
            touchedFieldsRef.current.add("description");
            setDescription(event.target.value);
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
