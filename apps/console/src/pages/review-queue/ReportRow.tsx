import { useState } from "react";

import { ApiError } from "../../api/client";
import type { IncidentType } from "../../api/cases";
import type { ReportSummary } from "../../api/reports";
import { useTranslation } from "../../i18n/LanguageContext";
import { ExtractionPanel } from "./ExtractionPanel";

const STATUS_STYLES: Record<ReportSummary["status"], string> = {
  RECEIVED: "bg-amber-500/10 text-amber-300",
  UNDER_REVIEW: "bg-blue-500/10 text-blue-300",
  LINKED_TO_CASE: "bg-emerald-500/10 text-emerald-300",
  CLOSED: "bg-white/[0.08] text-slate-400",
};

interface ReportRowProps {
  report: ReportSummary;
  token: string;
  onCreateCase: (reportId: string, incidentType: IncidentType) => Promise<void>;
  onLinkToCase: (reportId: string, caseId: string) => Promise<void>;
}

export function ReportRow({ report, token, onCreateCase, onLinkToCase }: ReportRowProps) {
  const { t } = useTranslation();
  const incidentTypes: { value: IncidentType; label: string }[] = [
    { value: "MISSING_CHILD", label: t.reportRow.incidentTypeMissingChild },
    { value: "OTHER_PROTECTION_INCIDENT", label: t.reportRow.incidentTypeOtherProtection },
  ];
  const [expanded, setExpanded] = useState(false);
  const [incidentType, setIncidentType] = useState<IncidentType>(
    report.reported_incident_type ?? "MISSING_CHILD",
  );
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
      const message = cause instanceof ApiError ? cause.message : t.common.unexpectedError;
      const requestId = cause instanceof ApiError ? cause.requestId : null;
      setError(requestId ? `${message} (reference: ${requestId})` : message);
    } finally {
      setPending(false);
    }
  }

  return (
    <tr className="border-b border-white/[0.06] align-top">
      <td className="py-3 pr-4 font-mono text-xs text-slate-500">{report.report_id.slice(0, 8)}</td>
      <td className="py-3 pr-4 text-sm text-slate-300">{report.source_channel}</td>
      <td className="py-3 pr-4">
        <span className={`rounded px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[report.status]}`}>
          {report.status.replace(/_/g, " ")}
        </span>
      </td>
      <td className="py-3 pr-4 text-sm text-slate-500">{new Date(report.received_at).toLocaleString()}</td>
      <td className="py-3 pr-4">
        <p className={expanded ? "whitespace-pre-wrap text-sm text-slate-300" : "truncate text-sm text-slate-300"}>
          {report.raw_content}
        </p>
        {report.raw_content.length > 80 && (
          <button
            type="button"
            onClick={() => setExpanded((value) => !value)}
            className="mt-1 text-xs font-medium text-slate-500 underline"
          >
            {expanded ? t.reportRow.showLess : t.reportRow.showMore}
          </button>
        )}

        <ExtractionPanel token={token} reportId={report.report_id} />

        {report.reported_incident_type && (
          <p className="mt-2 text-xs text-slate-400">
            {t.reportRow.reporterSuggested}{" "}
            {incidentTypes.find((option) => option.value === report.reported_incident_type)
              ?.label ?? report.reported_incident_type}
          </p>
        )}

        <div className="mt-3 flex flex-wrap items-center gap-2">
          <select
            value={incidentType}
            onChange={(event) => setIncidentType(event.target.value as IncidentType)}
            className="rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-xs text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
            aria-label={t.reportRow.incidentTypeLabel(report.report_id)}
          >
            {incidentTypes.map((option) => (
              <option key={option.value} value={option.value} className="bg-slate-900 text-white">
                {option.label}
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={pending}
            onClick={() => void runAction(() => onCreateCase(report.report_id, incidentType))}
            className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1 text-xs font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t.reportRow.createCase}
          </button>

          <input
            value={caseId}
            onChange={(event) => setCaseId(event.target.value)}
            placeholder={t.reportRow.existingCaseIdPlaceholder}
            aria-label={t.reportRow.existingCaseIdLabel(report.report_id)}
            className="rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-xs text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
          <button
            type="button"
            disabled={pending || !caseId.trim()}
            onClick={() => void runAction(() => onLinkToCase(report.report_id, caseId.trim()))}
            className="rounded-lg border border-white/[0.08] px-3 py-1 text-xs font-medium text-slate-300 hover:bg-white/[0.05] disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t.reportRow.linkToCase}
          </button>
        </div>

        {error && (
          <p role="alert" className="mt-2 text-xs text-red-300">
            {error}
          </p>
        )}
      </td>
    </tr>
  );
}
