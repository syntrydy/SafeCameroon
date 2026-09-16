import { Fragment, useCallback, useEffect, useState } from "react";

import { createExtraction, listExtractions, type Extraction } from "../../api/extractions";
import { ApiError } from "../../api/client";

const FIELD_LABELS: { key: keyof Extraction["fields"]; label: string }[] = [
  { key: "person_description", label: "Description" },
  { key: "age", label: "Age" },
  { key: "time", label: "Time" },
  { key: "place", label: "Place" },
  { key: "incident_category", label: "Suggested category" },
  { key: "vehicle_details", label: "Vehicle" },
  { key: "contact_request", label: "Contact request" },
];

function ExtractionFields({ fields }: { fields: Extraction["fields"] }) {
  const present = FIELD_LABELS.filter(({ key }) => fields[key]);
  if (present.length === 0) {
    return <p className="text-xs italic text-slate-400">No candidate details found.</p>;
  }
  return (
    <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
      {present.map(({ key, label }) => (
        <Fragment key={key}>
          <span className="font-medium text-slate-500">{label}</span>
          <span className="text-slate-700">{fields[key]}</span>
        </Fragment>
      ))}
    </div>
  );
}

export function ExtractionPanel({ token, reportId }: { token: string; reportId: string }) {
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
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setLoading(false);
    }
  }, [token, reportId]);

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
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setExtracting(false);
    }
  }

  if (!open) {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="mt-2 text-xs font-medium text-indigo-700 underline"
      >
        AI extraction
      </button>
    );
  }

  return (
    <div className="mt-2 rounded border border-indigo-100 bg-indigo-50/50 p-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold text-indigo-900">AI extraction</span>
        <button
          type="button"
          disabled={extracting}
          onClick={() => void handleExtract()}
          className="rounded bg-indigo-700 px-2 py-1 text-xs font-medium text-white disabled:opacity-50"
        >
          {extracting ? "Extracting..." : "Extract candidate info"}
        </button>
      </div>
      <p className="mt-1 text-xs text-slate-500">
        Unverified suggestion, not a fact -- confirm anything relevant yourself before acting on
        it.
      </p>

      {error && (
        <p role="alert" className="mt-2 text-xs text-red-700">
          {error}
        </p>
      )}

      {loading && <p className="mt-2 text-xs text-slate-400">Loading...</p>}

      {!loading && extractions.length === 0 && !error && (
        <p className="mt-2 text-xs text-slate-400">No extraction requested yet.</p>
      )}

      <ul className="mt-2 space-y-2">
        {extractions.map((extraction, index) => (
          <li key={index} className="rounded bg-white p-2 shadow-sm">
            <ExtractionFields fields={extraction.fields} />
            <p className="mt-1 text-[10px] text-slate-400">
              {extraction.provider.toLowerCase()}/{extraction.model}
            </p>
          </li>
        ))}
      </ul>
    </div>
  );
}
