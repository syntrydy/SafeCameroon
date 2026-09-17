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
        <h2 className="text-base font-semibold text-slate-900">{t.reviewQueue.heading}</h2>
        <label className="text-sm text-slate-600">
          {t.reviewQueue.statusLabel}{" "}
          <select
            value={statusFilter}
            onChange={(event) => {
              setActionMessage(null);
              setStatusFilter(event.target.value as ReportStatus | "ALL");
            }}
            className="ml-2 rounded border border-slate-300 px-2 py-1 text-sm"
          >
            {statusFilters.map((filter) => (
              <option key={filter.value} value={filter.value}>
                {filter.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      {actionMessage && (
        <p role="status" className="mb-4 rounded bg-emerald-50 px-3 py-2 text-sm text-emerald-700">
          {actionMessage.text}{" "}
          <Link to={`/cases/${actionMessage.caseId}`} className="underline">
            {t.reviewQueue.viewCase}
          </Link>
        </p>
      )}

      {error && (
        <p role="alert" className="mb-4 rounded bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      {loading ? (
        <p className="text-sm text-slate-500">{t.reviewQueue.loading}</p>
      ) : reports.length === 0 ? (
        <p className="text-sm text-slate-500">{t.reviewQueue.noReports}</p>
      ) : (
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-slate-200 text-xs uppercase text-slate-500">
              <th className="py-2 pr-4">{t.reviewQueue.colReport}</th>
              <th className="py-2 pr-4">{t.reviewQueue.colChannel}</th>
              <th className="py-2 pr-4">{t.reviewQueue.colStatus}</th>
              <th className="py-2 pr-4">{t.reviewQueue.colReceived}</th>
              <th className="py-2 pr-4">{t.reviewQueue.colContentActions}</th>
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
      )}
    </div>
  );
}
