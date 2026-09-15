import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";

import { listAlerts, type Alert, type AlertStatus } from "../../api/alerts";
import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";

const STATUS_FILTERS: { value: AlertStatus | "ALL"; label: string }[] = [
  { value: "ALL", label: "All" },
  { value: "ACTIVE", label: "Active" },
  { value: "CANCELLED", label: "Cancelled" },
];

export function AlertList() {
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>.
  const token = session!.token;

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
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setLoading(false);
    }
  }, [token, statusFilter]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div>
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-base font-semibold text-slate-900">Alerts</h2>
        <label className="text-sm text-slate-600">
          Status:{" "}
          <select
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value as AlertStatus | "ALL")}
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

      {error && (
        <p role="alert" className="mb-4 rounded bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      {loading ? (
        <p className="text-sm text-slate-500">Loading alerts...</p>
      ) : alerts.length === 0 ? (
        <p className="text-sm text-slate-500">No alerts match this filter.</p>
      ) : (
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-slate-200 text-xs uppercase text-slate-500">
              <th className="py-2 pr-4">Alert</th>
              <th className="py-2 pr-4">Severity</th>
              <th className="py-2 pr-4">Visibility</th>
              <th className="py-2 pr-4">Status</th>
              <th className="py-2 pr-4">Target geography</th>
            </tr>
          </thead>
          <tbody>
            {alerts.map((alert) => (
              <tr key={alert.alert_id} className="border-b border-slate-100">
                <td className="py-2 pr-4">
                  <Link to={`/alerts/${alert.alert_id}`} className="font-mono text-xs text-slate-700 underline">
                    {alert.alert_id.slice(0, 8)}
                  </Link>
                </td>
                <td className="py-2 pr-4 text-sm text-slate-700">{alert.severity}</td>
                <td className="py-2 pr-4 text-sm text-slate-700">{alert.visibility}</td>
                <td className="py-2 pr-4 text-sm text-slate-700">{alert.status}</td>
                <td className="py-2 pr-4 text-sm text-slate-700">{alert.target_geography}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
