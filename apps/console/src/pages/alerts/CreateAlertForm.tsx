import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";

import { generateAlertDescription } from "../../api/alertDescription";
import { createAlert, type Alert, type Severity } from "../../api/alerts";
import type { ExtractedFields } from "../../api/extractions";
import { listGeographyAreas } from "../../api/subscriptions";
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
  // Target geography is who gets notified (GeoArea::matches_target requires
  // the alert's target_geography to *contain* a subscriber's exact area
  // string), so it's built from areas real subscribers actually chose --
  // never free text a reviewer guesses at, which could silently match no
  // one -- except as a fallback when literally no one has subscribed to any
  // area yet (a fresh deployment's very first alert).
  const [knownAreas, setKnownAreas] = useState<string[]>([]);
  const [areasLoaded, setAreasLoaded] = useState(false);
  const [selectedAreas, setSelectedAreas] = useState<string[]>([]);
  const [areaInput, setAreaInput] = useState("");
  const [descriptionEn, setDescriptionEn] = useState("");
  const [descriptionFr, setDescriptionFr] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [suggestionsAppliedAt, setSuggestionsAppliedAt] = useState<number | null>(null);
  const [generatingDescription, setGeneratingDescription] = useState(false);
  // Model attribution for the caption under the generate button, mirroring
  // ExtractionPanel's `provider.toLowerCase()}/{model}` caption -- set once
  // a generation succeeds, cleared on a fresh attempt.
  const [descriptionGeneratedBy, setDescriptionGeneratedBy] = useState<{
    provider: string;
    model: string;
  } | null>(null);
  // Fields the reviewer has actually touched, tracked independently of
  // their current value (which may still be empty if they typed then
  // cleared it). A suggestion can arrive at any point -- e.g. auto-applied
  // the moment the page loads, which can race a reviewer already
  // typing/picking -- so "currently empty" alone isn't a safe signal that a
  // field is still untouched; this is checked instead of/alongside that.
  const touchedFieldsRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    let cancelled = false;
    listGeographyAreas(token)
      .then((areas) => {
        if (!cancelled) setKnownAreas(areas);
      })
      .catch(() => {
        // Best-effort: the picker still works as free-text entry (see
        // `noKnownAreasYet`) if this fails to load.
      })
      .finally(() => {
        if (!cancelled) setAreasLoaded(true);
      });
    return () => {
      cancelled = true;
    };
  }, [token]);

  useEffect(() => {
    if (!suggestedFields) return;
    if (!touchedFieldsRef.current.has("descriptionEn")) {
      const composed = composeAlertDescription(suggestedFields.fields);
      if (composed) {
        setDescriptionEn((current) => (current.trim() ? current : composed));
        // Immediately formalize+translate that mechanical join into a real
        // EN sentence and an actual FR translation, so applying an
        // extraction lands the reviewer on both languages ready to send
        // rather than a raw field dump they'd otherwise have to remember
        // to click "Generate formal description" on. Silent: this is a
        // background convenience, not a reviewer-initiated action, so a
        // failure (e.g. no AI provider configured) just leaves the
        // mechanical composed text as the EN fallback and FR empty --
        // exactly today's behavior before this auto-trigger existed --
        // rather than surfacing an error for something nobody asked for.
        void generateDescription(composed, { silent: true });
      }
    }
    setSuggestionsAppliedAt(suggestedFields.appliedAt);
    // Re-runs only when a *new* apply happens (a fresh appliedAt), not on
    // every render -- `suggestedFields` itself is a fresh object each time
    // the parent re-renders, which would re-fire this on unrelated updates.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [suggestedFields?.appliedAt]);

  // Separate from the description effect above because it also depends on
  // `knownAreas`, which can still be loading when a suggestion is first
  // applied (auto-apply on case load races this fetch) -- this re-evaluates
  // once the areas arrive rather than only firing once and missing them.
  useEffect(() => {
    const place = suggestedFields?.fields.place;
    if (!place || touchedFieldsRef.current.has("targetGeography")) return;
    const lowerPlace = place.toLowerCase();
    const matches = knownAreas.filter((area) => lowerPlace.includes(area.toLowerCase()));
    if (matches.length > 0) {
      setSelectedAreas((current) => (current.length > 0 ? current : matches));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [suggestedFields?.appliedAt, knownAreas]);

  function addArea(area: string) {
    const trimmed = area.trim();
    if (!trimmed) return;
    touchedFieldsRef.current.add("targetGeography");
    setSelectedAreas((current) => (current.includes(trimmed) ? current : [...current, trimmed]));
    setAreaInput("");
  }

  function removeArea(area: string) {
    touchedFieldsRef.current.add("targetGeography");
    setSelectedAreas((current) => current.filter((value) => value !== area));
  }

  const filteredSuggestions = knownAreas.filter(
    (area) =>
      !selectedAreas.includes(area) && area.toLowerCase().includes(areaInput.trim().toLowerCase()),
  );

  function handleAreaInputKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter") return;
    event.preventDefault();
    if (filteredSuggestions.length > 0) {
      addArea(filteredSuggestions[0]);
    } else if (knownAreas.length === 0 && areaInput.trim()) {
      // No one has subscribed to any area anywhere yet -- fall back to a
      // free-text area so a fresh deployment's first alert isn't blocked.
      addArea(areaInput);
    }
  }

  // Formalizes/translates `sourceText` into a formal EN+FR pair. Shared by
  // the manual "Generate formal description" button (explicit reviewer
  // action -- always overwrites both fields, surfaces a failure, the same
  // "asking again gets a fresh attempt" behavior as ExtractionPanel's
  // extract button) and the auto-apply effect above (`silent: true` --
  // only applies if the reviewer hasn't started typing in the meantime,
  // and never surfaces a failure since nothing was explicitly requested).
  async function generateDescription(sourceText: string, options: { silent?: boolean } = {}) {
    if (!sourceText.trim()) {
      return;
    }
    setGeneratingDescription(true);
    if (!options.silent) {
      setError(null);
    }
    try {
      const suggestion = await generateAlertDescription(token, caseId, sourceText);
      if (options.silent && touchedFieldsRef.current.has("descriptionEn")) {
        return;
      }
      touchedFieldsRef.current.add("descriptionEn");
      setDescriptionEn(suggestion.description_en);
      setDescriptionFr(suggestion.description_fr);
      setDescriptionGeneratedBy({ provider: suggestion.provider, model: suggestion.model });
    } catch (cause) {
      if (!options.silent) {
        setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
      }
    } finally {
      setGeneratingDescription(false);
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (selectedAreas.length === 0) {
      setError(t.createAlertForm.targetGeographyRequired);
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const fields = [
        descriptionEn.trim() && { field: "SAFE_DESCRIPTION_EN" as const, value: descriptionEn.trim() },
        descriptionFr.trim() && { field: "SAFE_DESCRIPTION_FR" as const, value: descriptionFr.trim() },
      ].filter((field): field is { field: "SAFE_DESCRIPTION_EN" | "SAFE_DESCRIPTION_FR"; value: string } =>
        Boolean(field),
      );
      const alert = await createAlert(token, caseId, {
        severity,
        targetGeography: selectedAreas.join(", "),
        fields,
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

        <div className="block text-sm">
          <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.targetGeography}</span>
          <div className="flex flex-wrap items-center gap-1 rounded-lg border border-white/[0.08] bg-white/[0.03] p-1.5 focus-within:border-emerald-500/50 focus-within:ring-2 focus-within:ring-emerald-500/20">
            {selectedAreas.map((area) => (
              <span
                key={area}
                className="flex items-center gap-1 rounded bg-emerald-500/10 px-2 py-0.5 text-xs text-emerald-300"
              >
                {area}
                <button
                  type="button"
                  onClick={() => removeArea(area)}
                  aria-label={t.createAlertForm.removeArea(area)}
                  className="text-emerald-400 hover:text-emerald-200"
                >
                  &times;
                </button>
              </span>
            ))}
            <input
              aria-label={t.createAlertForm.targetGeography}
              value={areaInput}
              onChange={(event) => setAreaInput(event.target.value)}
              onKeyDown={handleAreaInputKeyDown}
              placeholder={selectedAreas.length === 0 ? t.createAlertForm.targetGeographyPlaceholder : ""}
              className="min-w-[8rem] flex-1 bg-transparent text-sm text-white placeholder-slate-500 focus:outline-none"
            />
          </div>
          {areaInput.trim() && filteredSuggestions.length > 0 && (
            <ul className="mt-1 max-h-40 overflow-auto rounded-lg border border-white/[0.08] bg-slate-900 text-sm">
              {filteredSuggestions.map((area) => (
                <li key={area}>
                  <button
                    type="button"
                    onClick={() => addArea(area)}
                    className="block w-full px-2 py-1 text-left text-slate-200 hover:bg-white/[0.06]"
                  >
                    {area}
                  </button>
                </li>
              ))}
            </ul>
          )}
          {areasLoaded && knownAreas.length === 0 && (
            <p className="mt-1 text-xs text-slate-500">{t.createAlertForm.noKnownAreasYet}</p>
          )}
        </div>
      </div>

      <label className="mt-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.descriptionEn}</span>
        <textarea
          required
          rows={5}
          value={descriptionEn}
          onChange={(event) => {
            touchedFieldsRef.current.add("descriptionEn");
            setDescriptionEn(event.target.value);
          }}
          className={fieldInputClassName}
        />
      </label>

      <div className="mt-2 flex items-center gap-2">
        <button
          type="button"
          disabled={generatingDescription}
          onClick={() => {
            const sourceText =
              descriptionEn.trim() || (suggestedFields ? composeAlertDescription(suggestedFields.fields) : "");
            void generateDescription(sourceText);
          }}
          className="rounded-lg border border-white/[0.08] px-2 py-1 text-xs font-medium text-slate-300 hover:bg-white/[0.05] disabled:cursor-not-allowed disabled:opacity-50"
        >
          {generatingDescription ? t.createAlertForm.generatingDescription : t.createAlertForm.generateDescription}
        </button>
        {descriptionGeneratedBy && (
          <span className="text-[10px] text-slate-500">
            {descriptionGeneratedBy.provider.toLowerCase()}/{descriptionGeneratedBy.model}
          </span>
        )}
      </div>

      <label className="mt-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-300">{t.createAlertForm.descriptionFr}</span>
        <textarea
          required
          rows={5}
          value={descriptionFr}
          onChange={(event) => {
            touchedFieldsRef.current.add("descriptionFr");
            setDescriptionFr(event.target.value);
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
