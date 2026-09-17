import { Fragment, useCallback, useEffect, useState } from "react";

import { createExtraction, listExtractions, type Extraction } from "../../api/extractions";
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

export function ExtractionPanel({ token, reportId }: { token: string; reportId: string }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [extractions, setExtractions] = useState<Extraction[]>([]);
  const [loading, setLoading] = useState(false);
  const [extracting, setExtracting] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  async function handleExtract() {
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
  }

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
            <p className="mt-1 text-[10px] text-slate-400">
              {extraction.provider.toLowerCase()}/{extraction.model}
            </p>
          </li>
        ))}
      </ul>
    </div>
  );
}
