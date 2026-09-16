import { useState } from "react";

import { ApiError } from "../../api/client";
import type { IncidentType } from "../../api/cases";
import type { ReportSummary } from "../../api/reports";
import { ExtractionPanel } from "./ExtractionPanel";

const INCIDENT_TYPES: { value: IncidentType; label: string }[] = [
  { value: "MISSING_CHILD", label: "Missing child" },
  { value: "OTHER_PROTECTION_INCIDENT", label: "Other protection incident" },
];

const STATUS_STYLES: Record<ReportSummary["status"], string> = {
  RECEIVED: "bg-amber-100 text-amber-800",
  UNDER_REVIEW: "bg-blue-100 text-blue-800",
  LINKED_TO_CASE: "bg-emerald-100 text-emerald-800",
  CLOSED: "bg-slate-200 text-slate-600",
};

interface ReportRowProps {
  report: ReportSummary;
  token: string;
  onCreateCase: (reportId: string, incidentType: IncidentType) => Promise<void>;
  onLinkToCase: (reportId: string, caseId: string) => Promise<void>;
}

export function ReportRow({ report, token, onCreateCase, onLinkToCase }: ReportRowProps) {
  const [expanded, setExpanded] = useState(false);
  const [incidentType, setIncidentType] = useState<IncidentType>("MISSING_CHILD");
  const [caseId, setCaseId] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // A successful action re-fetches the queue, which can remove this row
  // immediately (e.g. a created case takes the report out of "received").
  // Success feedback therefore lives on the parent page, not here — only
  // failures, which leave the row in place, are shown inline.
  async function runAction(action: () => Promise<void>) {
    setPending(true);
    setError(null);
    try {
      await action();
    } catch (cause) {
      const message = cause instanceof ApiError ? cause.message : "An unexpected error occurred.";
      const requestId = cause instanceof ApiError ? cause.requestId : null;
      setError(requestId ? `${message} (reference: ${requestId})` : message);
    } finally {
      setPending(false);
    }
  }

  return (
    <tr className="border-b border-slate-100 align-top">
      <td className="py-3 pr-4 font-mono text-xs text-slate-500">{report.report_id.slice(0, 8)}</td>
      <td className="py-3 pr-4 text-sm text-slate-700">{report.source_channel}</td>
      <td className="py-3 pr-4">
        <span className={`rounded px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[report.status]}`}>
          {report.status.replace(/_/g, " ")}
        </span>
      </td>
      <td className="py-3 pr-4 text-sm text-slate-500">{new Date(report.received_at).toLocaleString()}</td>
      <td className="py-3 pr-4">
        <p className={expanded ? "whitespace-pre-wrap text-sm text-slate-700" : "truncate text-sm text-slate-700"}>
          {report.raw_content}
        </p>
        {report.raw_content.length > 80 && (
          <button
            type="button"
            onClick={() => setExpanded((value) => !value)}
            className="mt-1 text-xs font-medium text-slate-500 underline"
          >
            {expanded ? "Show less" : "Show more"}
          </button>
        )}

        <ExtractionPanel token={token} reportId={report.report_id} />

        <div className="mt-3 flex flex-wrap items-center gap-2">
          <select
            value={incidentType}
            onChange={(event) => setIncidentType(event.target.value as IncidentType)}
            className="rounded border border-slate-300 px-2 py-1 text-xs"
            aria-label={`Incident type for report ${report.report_id}`}
          >
            {INCIDENT_TYPES.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={pending}
            onClick={() => void runAction(() => onCreateCase(report.report_id, incidentType))}
            className="rounded bg-slate-900 px-3 py-1 text-xs font-medium text-white disabled:opacity-50"
          >
            Create case
          </button>

          <input
            value={caseId}
            onChange={(event) => setCaseId(event.target.value)}
            placeholder="Existing case id"
            aria-label={`Existing case id for report ${report.report_id}`}
            className="rounded border border-slate-300 px-2 py-1 text-xs"
          />
          <button
            type="button"
            disabled={pending || !caseId.trim()}
            onClick={() => void runAction(() => onLinkToCase(report.report_id, caseId.trim()))}
            className="rounded border border-slate-300 px-3 py-1 text-xs font-medium text-slate-700 disabled:opacity-50"
          >
            Link to case
          </button>
        </div>

        {error && (
          <p role="alert" className="mt-2 text-xs text-red-700">
            {error}
          </p>
        )}
      </td>
    </tr>
  );
}
