import { useCallback, useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { cancelAlert, getAlert, type Alert } from "../../api/alerts";
import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";
import type { Translations } from "../../i18n/translations";

function visibilityLabels(t: Translations): Record<Alert["visibility"], string> {
  return {
    INTERNAL: t.alertPreview.visibilityInternal,
    PARTNER: t.alertPreview.visibilityPartner,
    COMMUNITY: t.alertPreview.visibilityCommunity,
    PUBLIC: t.alertPreview.visibilityPublic,
  };
}

// This is the alert's actual outbound safe projection -- exactly the fields
// its policy allowed, nothing more (docs/ALERT_SAFETY.md). There is
// deliberately no way to view raw case/report content from this screen.
export function AlertPreview() {
  const { t } = useTranslation();
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
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setLoading(false);
    }
  }, [token, id, t]);

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
      setActionMessage(t.alertPreview.alertCancelled);
      await load();
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setCancelling(false);
    }
  }

  if (loading) {
    return <p className="text-sm text-slate-500">{t.alertPreview.loading}</p>;
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
          <h2 className="text-base font-semibold text-slate-900">
            {t.alertPreview.heading(alert.alert_id.slice(0, 8))}
          </h2>
          <p className="text-sm text-slate-500">
            {alert.severity} &middot; {visibilityLabels(t)[alert.visibility]} &middot;{" "}
            {t.alertPreview.version(alert.version)}
          </p>
        </div>
        <span
          className={`rounded px-2 py-1 text-xs font-medium ${
            alert.status === "ACTIVE" ? "bg-emerald-100 text-emerald-800" : "bg-slate-200 text-slate-600"
          }`}
        >
          {alert.status === "ACTIVE" ? t.alertPreview.statusActive : t.alertPreview.statusCancelled}
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
        <dt className="text-slate-500">{t.alertPreview.targetGeography}</dt>
        <dd className="text-slate-900">{alert.target_geography}</dd>
        <dt className="text-slate-500">{t.alertPreview.caseLabel}</dt>
        <dd className="text-slate-900">{alert.case_id.slice(0, 8)}</dd>
        <dt className="text-slate-500">{t.alertPreview.policyLabel}</dt>
        <dd className="text-slate-900">
          {alert.policy_id} v{alert.policy_version}
        </dd>
        <dt className="text-slate-500">{t.alertPreview.deliveriesLabel}</dt>
        <dd className="text-slate-900">
          <Link to={`/alerts/${alert.alert_id}/deliveries`} className="underline">
            {t.alertPreview.viewDeliveries}
          </Link>
        </dd>
      </dl>

      <section className="mb-6">
        <h3 className="mb-2 text-sm font-semibold text-slate-900">{t.alertPreview.safeProjection}</h3>
        {alert.fields.length === 0 ? (
          <p className="text-sm text-slate-500">{t.alertPreview.noFields}</p>
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
          {t.alertPreview.cancelAlert}
        </button>
      )}
    </div>
  );
}
