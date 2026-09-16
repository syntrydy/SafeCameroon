import { useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { cancelAlert, getAlert, type Alert } from "../../api/alerts";
import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";

const VISIBILITY_LABELS: Record<Alert["visibility"], string> = {
  INTERNAL: "Internal",
  PARTNER: "Partner",
  COMMUNITY: "Community",
  PUBLIC: "Public",
};

// This is the alert's actual outbound safe projection -- exactly the fields
// its policy allowed, nothing more (docs/ALERT_SAFETY.md). There is
// deliberately no way to view raw case/report content from this screen.
export function AlertPreview() {
  const { id } = useParams<{ id: string }>();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>.
  const token = session!.token;

  const [alert, setAlert] = useState<Alert | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);

  const load = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    setError(null);
    try {
      setAlert(await getAlert(token, id));
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setLoading(false);
    }
  }, [token, id]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleCancel() {
    if (!id) return;
    setCancelling(true);
    setActionMessage(null);
    setError(null);
    try {
      await cancelAlert(token, id);
      setActionMessage("Alert cancelled.");
      await load();
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setCancelling(false);
    }
  }

  if (loading) {
    return <p className="text-sm text-slate-500">Loading alert...</p>;
  }

  if (error && !alert) {
    return (
      <p role="alert" className="rounded bg-red-50 px-3 py-2 text-sm text-red-700">
        {error}
      </p>
    );
  }

  if (!alert) {
    return null;
  }

  return (
    <div>
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h2 className="text-base font-semibold text-slate-900">Alert {alert.alert_id.slice(0, 8)}</h2>
          <p className="text-sm text-slate-500">
            {alert.severity} &middot; {VISIBILITY_LABELS[alert.visibility]} &middot; version {alert.version}
          </p>
        </div>
        <span
          className={`rounded px-2 py-1 text-xs font-medium ${
            alert.status === "ACTIVE" ? "bg-emerald-100 text-emerald-800" : "bg-slate-200 text-slate-600"
          }`}
        >
          {alert.status === "ACTIVE" ? "Active" : "Cancelled"}
        </span>
      </div>

      {actionMessage && (
        <p role="status" className="mb-4 rounded bg-emerald-50 px-3 py-2 text-sm text-emerald-700">
          {actionMessage}
        </p>
      )}
      {error && (
        <p role="alert" className="mb-4 rounded bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      <dl className="mb-6 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="text-slate-500">Target geography</dt>
        <dd className="text-slate-900">{alert.target_geography}</dd>
        <dt className="text-slate-500">Case</dt>
        <dd className="text-slate-900">{alert.case_id.slice(0, 8)}</dd>
        <dt className="text-slate-500">Policy</dt>
        <dd className="text-slate-900">
          {alert.policy_id} v{alert.policy_version}
        </dd>
        <dt className="text-slate-500">Deliveries</dt>
        <dd className="text-slate-900">
          <Link to={`/alerts/${alert.alert_id}/deliveries`} className="underline">
            View deliveries
          </Link>
        </dd>
      </dl>

      <section className="mb-6">
        <h3 className="mb-2 text-sm font-semibold text-slate-900">Safe projection</h3>
        {alert.fields.length === 0 ? (
          <p className="text-sm text-slate-500">No fields were included.</p>
        ) : (
          <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
            {alert.fields.map((field) => (
              <div key={field.field} className="contents">
                <dt className="text-slate-500">{field.field.replace(/_/g, " ")}</dt>
                <dd className="text-slate-900">{field.value}</dd>
              </div>
            ))}
          </dl>
        )}
      </section>

      {alert.status === "ACTIVE" && (
        <button
          type="button"
          disabled={cancelling}
          onClick={() => void handleCancel()}
          className="rounded border border-red-300 px-3 py-1.5 text-sm font-medium text-red-700 disabled:opacity-50"
        >
          Cancel alert
        </button>
      )}
    </div>
  );
}
