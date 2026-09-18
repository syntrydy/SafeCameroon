import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { listAlerts, type Alert, type AlertStatus } from "../../api/alerts";
import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";

export function AlertList() {
  const { t } = useTranslation();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>.
  const token = session!.token;

  const statusFilters: { value: AlertStatus | "ALL"; label: string }[] = [
    { value: "ALL", label: t.alertList.statusAll },
    { value: "ACTIVE", label: t.alertList.statusActive },
    { value: "CANCELLED", label: t.alertList.statusCancelled },
  ];

  const [statusFilter, setStatusFilter] = useState<AlertStatus | "ALL">("ACTIVE");
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setAlerts(await listAlerts(token, { status: statusFilter === "ALL" ? undefined : statusFilter }));
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
        <h1 className="text-base font-semibold text-white">{t.alertList.heading}</h1>
        <label className="text-sm text-slate-400">
          {t.alertList.statusLabel}{" "}
          <select
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value as AlertStatus | "ALL")}
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
        <p className="text-sm text-slate-500">{t.alertList.loading}</p>
      ) : alerts.length === 0 ? (
        <p className="text-sm text-slate-500">{t.alertList.noAlerts}</p>
      ) : (
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-white/[0.08] text-xs uppercase text-slate-500">
              <th className="py-2 pr-4">{t.alertList.colAlert}</th>
              <th className="py-2 pr-4">{t.alertList.colSeverity}</th>
              <th className="py-2 pr-4">{t.alertList.colVisibility}</th>
              <th className="py-2 pr-4">{t.alertList.colStatus}</th>
              <th className="py-2 pr-4">{t.alertList.colTargetGeography}</th>
            </tr>
          </thead>
          <tbody>
            {alerts.map((alert) => (
              <tr key={alert.alert_id} className="border-b border-white/[0.06]">
                <td className="py-2 pr-4">
                  <Link to={`/alerts/${alert.alert_id}`} className="font-mono text-xs text-slate-300 underline">
                    {alert.alert_id.slice(0, 8)}
                  </Link>
                </td>
                <td className="py-2 pr-4 text-sm text-slate-300">{alert.severity}</td>
                <td className="py-2 pr-4 text-sm text-slate-300">{alert.visibility}</td>
                <td className="py-2 pr-4 text-sm text-slate-300">{alert.status}</td>
                <td className="py-2 pr-4 text-sm text-slate-300">{alert.target_geography}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
