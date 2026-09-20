import { Fragment, useCallback, useEffect, useRef, useState } from "react";

import { createExtraction, listExtractions, type Extraction, type ExtractedFields } from "../../api/extractions";
import { ApiError } from "../../api/client";
import { useTranslation } from "../../i18n/LanguageContext";
import type { Translations } from "../../i18n/translations";

function fieldLabels(t: Translations): { key: keyof Extraction["fields"]; label: string }[] {
  return [
    { key: "person_description", label: t.extraction.fieldDescription },
    { key: "age", label: t.extraction.fieldAge },
    { key: "time", label: t.extraction.fieldTime },
    { key: "place", label: t.extraction.fieldPlace },
    { key: "incident_category", label: t.extraction.fieldIncidentCategory },
    { key: "vehicle_details", label: t.extraction.fieldVehicleDetails },
    { key: "contact_request", label: t.extraction.fieldContactRequest },
  ];
}

function ExtractionFields({ fields, t }: { fields: Extraction["fields"]; t: Translations }) {
  const present = fieldLabels(t).filter(({ key }) => fields[key]);
  if (present.length === 0) {
    return <p className="text-xs italic text-slate-400">{t.extraction.noCandidateDetails}</p>;
  }
  return (
    <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
      {present.map(({ key, label }) => (
        <Fragment key={key}>
          <span className="font-medium text-slate-500">{label}</span>
          <span className="text-slate-300">{fields[key]}</span>
        </Fragment>
      ))}
    </div>
  );
}

interface ExtractionPanelProps {
  token: string;
  reportId: string;
  // When set, each listed extraction gets an "apply" action that hands its
  // fields to the caller instead of only displaying them read-only --
  // used by the case detail page to prefill the create-alert form. Omitted
  // entirely on the review queue, where there is no form to fill yet.
  onApply?: (fields: ExtractedFields) => void;
  // When true (the case detail page's *first* linked report, only while the
  // create-alert form is actually rendered), this panel opens, fetches this
  // report's extractions on mount, creates one if none exist yet, and hands
  // the most recent one to `onApply` automatically -- once -- so the form
  // starts pre-filled without the reviewer needing to find and click
  // through this panel first. Fields stay fully editable; this only saves
  // the extra clicks to see the AI's suggestion in the first place.
  autoApply?: boolean;
}

export function ExtractionPanel({ token, reportId, onApply, autoApply }: ExtractionPanelProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(Boolean(autoApply));
  const [extractions, setExtractions] = useState<Extraction[]>([]);
  // Starts `true` when auto-applying, so the effect below never sees a
  // "not loading, no extractions yet" state before the initial fetch (kicked
  // off by the `open`-load effect below) has actually run -- otherwise it
  // would create a redundant duplicate extraction racing that fetch.
  const [loading, setLoading] = useState(Boolean(autoApply));
  const [extracting, setExtracting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const hasAutoAppliedRef = useRef(false);
  const hasAutoExtractedRef = useRef(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setExtractions(await listExtractions(token, reportId));
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setLoading(false);
    }
  }, [token, reportId, t]);

  useEffect(() => {
    if (open) {
      void load();
    }
  }, [open, load]);

  const handleExtract = useCallback(async () => {
    setExtracting(true);
    setError(null);
    try {
      const created = await createExtraction(token, reportId);
      setExtractions((current) => [created, ...current]);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setExtracting(false);
    }
  }, [token, reportId, t]);

  useEffect(() => {
    if (!autoApply || !onApply || loading || hasAutoAppliedRef.current) return;
    if (extractions.length > 0) {
      hasAutoAppliedRef.current = true;
      onApply(extractions[0].fields);
    } else if (!hasAutoExtractedRef.current) {
      hasAutoExtractedRef.current = true;
      void handleExtract();
    }
  }, [autoApply, onApply, loading, extractions, handleExtract]);

  if (!open) {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="mt-2 text-xs font-medium text-indigo-400 underline"
      >
        {t.extraction.toggle}
      </button>
    );
  }

  return (
    <div className="mt-2 rounded border border-indigo-500/20 bg-indigo-500/10 p-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold text-indigo-300">{t.extraction.toggle}</span>
        <button
          type="button"
          disabled={extracting}
          onClick={() => void handleExtract()}
          className="rounded bg-indigo-700 px-2 py-1 text-xs font-medium text-white disabled:opacity-50"
        >
          {extracting ? t.extraction.extracting : t.extraction.extractButton}
        </button>
      </div>
      <p className="mt-1 text-xs text-slate-500">{t.extraction.disclaimer}</p>

      {error && (
        <p role="alert" className="mt-2 text-xs text-red-300">
          {error}
        </p>
      )}

      {loading && <p className="mt-2 text-xs text-slate-400">{t.extraction.loading}</p>}

      {!loading && extractions.length === 0 && !error && (
        <p className="mt-2 text-xs text-slate-400">{t.extraction.none}</p>
      )}

      <ul className="mt-2 space-y-2">
        {extractions.map((extraction, index) => (
          <li key={index} className="rounded bg-white/[0.05] p-2">
            <ExtractionFields fields={extraction.fields} t={t} />
            <div className="mt-1 flex items-center justify-between">
              <p className="text-[10px] text-slate-400">
                {extraction.provider.toLowerCase()}/{extraction.model}
              </p>
              {onApply && (
                <button
                  type="button"
                  onClick={() => onApply(extraction.fields)}
                  className="text-[10px] font-medium text-indigo-300 underline"
                >
                  {t.extraction.applyButton}
                </button>
              )}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
