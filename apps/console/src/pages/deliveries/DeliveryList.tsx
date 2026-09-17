import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { listDeliveriesForAlert, type Delivery, type DeliveryStatus } from "../../api/deliveries";
import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";

const STATUS_STYLES: Record<DeliveryStatus, string> = {
  QUEUED: "bg-slate-200 text-slate-600",
  SENDING: "bg-blue-100 text-blue-800",
  SENT: "bg-blue-100 text-blue-800",
  DELIVERED: "bg-emerald-100 text-emerald-800",
  RETRYING: "bg-amber-100 text-amber-800",
  FAILED_PERMANENTLY: "bg-red-100 text-red-800",
};

export function DeliveryList() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>.
  const token = session!.token;

  const [deliveries, setDeliveries] = useState<Delivery[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    setError(null);
    listDeliveriesForAlert(token, id)
      .then(setDeliveries)
      .catch((cause) => setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError))
      .finally(() => setLoading(false));
  }, [token, id, t]);

  return (
    <div>
      <h2 className="mb-4 text-base font-semibold text-slate-900">
        {t.deliveryList.heading(id?.slice(0, 8) ?? "")}
      </h2>

      {error && (
        <p role="alert" className="mb-4 rounded bg-red-50 px-3 py-2 text-sm text-red-700">
          {error}
        </p>
      )}

      {loading ? (
        <p className="text-sm text-slate-500">{t.deliveryList.loading}</p>
      ) : deliveries.length === 0 ? (
        <p className="text-sm text-slate-500">{t.deliveryList.none}</p>
      ) : (
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-slate-200 text-xs uppercase text-slate-500">
              <th className="py-2 pr-4">{t.deliveryList.colDelivery}</th>
              <th className="py-2 pr-4">{t.deliveryList.colChannel}</th>
              <th className="py-2 pr-4">{t.deliveryList.colTier}</th>
              <th className="py-2 pr-4">{t.deliveryList.colStatus}</th>
              <th className="py-2 pr-4">{t.deliveryList.colAttempts}</th>
            </tr>
          </thead>
          <tbody>
            {deliveries.map((delivery) => (
              <tr key={delivery.delivery_id} className="border-b border-slate-100">
                <td className="py-2 pr-4">
                  <Link
                    to={`/deliveries/${delivery.delivery_id}`}
                    className="font-mono text-xs text-slate-700 underline"
                  >
                    {delivery.delivery_id.slice(0, 8)}
                  </Link>
                </td>
                <td className="py-2 pr-4 text-sm text-slate-700">{delivery.channel}</td>
                <td className="py-2 pr-4 text-sm text-slate-700">{delivery.tier}</td>
                <td className="py-2 pr-4">
                  <span className={`rounded px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[delivery.status]}`}>
                    {delivery.status.replace(/_/g, " ")}
                  </span>
                </td>
                <td className="py-2 pr-4 text-sm text-slate-700">
                  {delivery.attempt_count} / {delivery.max_attempts}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
