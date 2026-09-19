import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { createCase, linkReportToCase, type IncidentType } from "../../api/cases";
import { ApiError } from "../../api/client";
import { listReports, type ReportStatus, type ReportSummary } from "../../api/reports";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";
import { ReportRow } from "./ReportRow";

export function ReviewQueue() {
  const { t } = useTranslation();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>, which redirects to
  // /login whenever there is no session.
  const token = session!.token;

  const statusFilters: { value: ReportStatus | "ALL"; label: string }[] = [
    { value: "ALL", label: t.reviewQueue.statusAll },
    { value: "RECEIVED", label: t.reviewQueue.statusReceived },
    { value: "UNDER_REVIEW", label: t.reviewQueue.statusUnderReview },
    { value: "LINKED_TO_CASE", label: t.reviewQueue.statusLinkedToCase },
    { value: "CLOSED", label: t.reviewQueue.statusClosed },
  ];

  const [statusFilter, setStatusFilter] = useState<ReportStatus | "ALL">("RECEIVED");
  const [reports, setReports] = useState<ReportSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<{ text: string; caseId: string } | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await listReports(token, {
        status: statusFilter === "ALL" ? undefined : statusFilter,
      });
      setReports(result);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setLoading(false);
    }
  }, [token, statusFilter, t]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCreateCase(reportId: string, incidentType: IncidentType) {
    const created = await createCase(token, reportId, incidentType);
    setActionMessage({ text: t.reviewQueue.caseCreated, caseId: created.case_id });
    await load();
  }

  async function handleLinkToCase(reportId: string, caseId: string) {
    const linked = await linkReportToCase(token, caseId, reportId);
    setActionMessage({ text: t.reviewQueue.linkedToCase, caseId: linked.case_id });
    await load();
  }

  return (
    <div>
      <div className="mb-4 flex items-center justify-between">
        <h1 className="text-base font-semibold text-white">{t.reviewQueue.heading}</h1>
        <label className="text-sm text-slate-400">
          {t.reviewQueue.statusLabel}{" "}
          <select
            value={statusFilter}
            onChange={(event) => {
              setActionMessage(null);
              setStatusFilter(event.target.value as ReportStatus | "ALL");
            }}
            className="ml-2 rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          >
            {statusFilters.map((filter) => (
              <option key={filter.value} value={filter.value} className="bg-slate-900 text-white">
                {filter.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      {actionMessage && (
        <p role="status" className="mb-4 rounded border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300">
          {actionMessage.text}{" "}
          <Link to={`/cases/${actionMessage.caseId}`} className="underline">
            {t.reviewQueue.viewCase}
          </Link>
        </p>
      )}

      {error && (
        <p role="alert" className="mb-4 rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
          {error}
        </p>
      )}

      {loading ? (
        <p className="text-sm text-slate-500">{t.reviewQueue.loading}</p>
      ) : reports.length === 0 ? (
        <p className="text-sm text-slate-500">{t.reviewQueue.noReports}</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-left">
            <thead>
              <tr className="border-b border-white/[0.08] text-xs uppercase text-slate-500">
                <th className="py-2 pr-4">{t.reviewQueue.colReport}</th>
                <th className="py-2 pr-4">{t.reviewQueue.colChannel}</th>
                <th className="py-2 pr-4">{t.reviewQueue.colStatus}</th>
                <th className="py-2 pr-4">{t.reviewQueue.colReceived}</th>
                <th className="py-2 pr-4">{t.reviewQueue.colContent}</th>
                <th className="py-2 pr-4">{t.reviewQueue.colActions}</th>
              </tr>
            </thead>
            <tbody>
              {reports.map((report) => (
                <ReportRow
                  key={report.report_id}
                  report={report}
                  token={token}
                  onCreateCase={handleCreateCase}
                  onLinkToCase={handleLinkToCase}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
