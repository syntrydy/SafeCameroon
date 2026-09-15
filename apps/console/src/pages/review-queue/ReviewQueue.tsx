import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { createCase, linkReportToCase, type IncidentType } from "../../api/cases";
import { ApiError } from "../../api/client";
import { listReports, type ReportStatus, type ReportSummary } from "../../api/reports";
import { useAuth } from "../../auth/AuthContext";
import { ReportRow } from "./ReportRow";

const STATUS_FILTERS: { value: ReportStatus | "ALL"; label: string }[] = [
  { value: "ALL", label: "All" },
  { value: "RECEIVED", label: "Received" },
  { value: "UNDER_REVIEW", label: "Under review" },
  { value: "LINKED_TO_CASE", label: "Linked to case" },
  { value: "CLOSED", label: "Closed" },
];

export function ReviewQueue() {
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>, which redirects to
  // /login whenever there is no session.
  const token = session!.token;

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
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setLoading(false);
    }
  }, [token, statusFilter]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCreateCase(reportId: string, incidentType: IncidentType) {
    const created = await createCase(token, reportId, incidentType);
    setActionMessage({ text: "Case created.", caseId: created.case_id });
    await load();
  }

  async function handleLinkToCase(reportId: string, caseId: string) {
    const linked = await linkReportToCase(token, caseId, reportId);
    setActionMessage({ text: "Linked to case.", caseId: linked.case_id });
    await load();
  }

  return (
    <div>
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-base font-semibold text-slate-900">Review queue</h2>
        <label className="text-sm text-slate-600">
          Status:{" "}
          <select
            value={statusFilter}
            onChange={(event) => {
              setActionMessage(null);
              setStatusFilter(event.target.value as ReportStatus | "ALL");
            }}
            className="ml-2 rounded border border-slate-300 px-2 py-1 text-sm"
          >
            {STATUS_FILTERS.map((filter) => (
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
            View case
          </Link>
        </p>
      )}

      {error && (
        <p role="alert" className="mb-4 rounded bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      {loading ? (
        <p className="text-sm text-slate-500">Loading reports...</p>
      ) : reports.length === 0 ? (
        <p className="text-sm text-slate-500">No reports match this filter.</p>
      ) : (
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-slate-200 text-xs uppercase text-slate-500">
              <th className="py-2 pr-4">Report</th>
              <th className="py-2 pr-4">Channel</th>
              <th className="py-2 pr-4">Status</th>
              <th className="py-2 pr-4">Received</th>
              <th className="py-2 pr-4">Content &amp; actions</th>
            </tr>
          </thead>
          <tbody>
            {reports.map((report) => (
              <ReportRow
                key={report.report_id}
                report={report}
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
