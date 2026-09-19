import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { listCases, type Case, type CaseStatus } from "../../api/cases";
import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";
import type { Translations } from "../../i18n/translations";

function incidentTypeLabel(incidentType: Case["incident_type"], t: Translations): string {
  switch (incidentType) {
    case "MISSING_CHILD":
      return t.caseList.incidentTypeMissingChild;
    case "OTHER_PROTECTION_INCIDENT":
      return t.caseList.incidentTypeOtherProtection;
  }
}

export function CaseList() {
  const { t } = useTranslation();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>.
  const token = session!.token;

  const statusFilters: { value: CaseStatus | "ALL"; label: string }[] = [
    { value: "ALL", label: t.caseList.statusAll },
    { value: "REPORTED", label: t.caseList.statusReported },
    { value: "UNDER_REVIEW", label: t.caseList.statusUnderReview },
    { value: "VERIFIED", label: t.caseList.statusVerified },
    { value: "ACTIVE", label: t.caseList.statusActive },
    { value: "RESOLVED", label: t.caseList.statusResolved },
    { value: "CANCELLED", label: t.caseList.statusCancelled },
    { value: "REJECTED", label: t.caseList.statusRejected },
  ];

  const [statusFilter, setStatusFilter] = useState<CaseStatus | "ALL">("ALL");
  const [cases, setCases] = useState<Case[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setCases(await listCases(token, { status: statusFilter === "ALL" ? undefined : statusFilter }));
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setLoading(false);
    }
  }, [token, statusFilter, t]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div>
      <div className="mb-4 flex items-center justify-between">
        <h1 className="text-base font-semibold text-white">{t.caseList.heading}</h1>
        <label className="text-sm text-slate-400">
          {t.caseList.statusLabel}{" "}
          <select
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value as CaseStatus | "ALL")}
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

      {error && (
        <p role="alert" className="mb-4 rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
          {error}
        </p>
      )}

      {loading ? (
        <p className="text-sm text-slate-500">{t.caseList.loading}</p>
      ) : cases.length === 0 ? (
        <p className="text-sm text-slate-500">{t.caseList.noCases}</p>
      ) : (
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-white/[0.08] text-xs uppercase text-slate-500">
              <th className="py-2 pr-4">{t.caseList.colCase}</th>
              <th className="py-2 pr-4">{t.caseList.colIncidentType}</th>
              <th className="py-2 pr-4">{t.caseList.colStatus}</th>
              <th className="py-2 pr-4">{t.caseList.colReports}</th>
            </tr>
          </thead>
          <tbody>
            {cases.map((caseItem) => (
              <tr key={caseItem.case_id} className="border-b border-white/[0.06]">
                <td className="py-2 pr-4">
                  <Link to={`/cases/${caseItem.case_id}`} className="font-mono text-xs text-slate-300 underline">
                    {caseItem.case_id.slice(0, 8)}
                  </Link>
                </td>
                <td className="py-2 pr-4 text-sm text-slate-300">{incidentTypeLabel(caseItem.incident_type, t)}</td>
                <td className="py-2 pr-4 text-sm text-slate-300">{caseItem.status.replace(/_/g, " ")}</td>
                <td className="py-2 pr-4 text-sm text-slate-300">{caseItem.report_ids.length}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
