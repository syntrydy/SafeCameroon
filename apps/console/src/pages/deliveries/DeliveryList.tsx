import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";

import { listDeliveriesForAlert, type Delivery, type DeliveryStatus } from "../../api/deliveries";
import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";

const STATUS_STYLES: Record<DeliveryStatus, string> = {
  QUEUED: "bg-white/[0.08] text-slate-400",
  SENDING: "bg-blue-500/10 text-blue-300",
  SENT: "bg-blue-500/10 text-blue-300",
  DELIVERED: "bg-emerald-500/10 text-emerald-300",
  RETRYING: "bg-amber-500/10 text-amber-300",
  FAILED_PERMANENTLY: "bg-red-500/10 text-red-300",
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
      <h1 className="mb-4 text-base font-semibold text-white">
        {t.deliveryList.heading(id?.slice(0, 8) ?? "")}
      </h1>

      {error && (
        <p role="alert" className="mb-4 rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
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
            <tr className="border-b border-white/[0.08] text-xs uppercase text-slate-500">
              <th className="py-2 pr-4">{t.deliveryList.colDelivery}</th>
              <th className="py-2 pr-4">{t.deliveryList.colChannel}</th>
              <th className="py-2 pr-4">{t.deliveryList.colTier}</th>
              <th className="py-2 pr-4">{t.deliveryList.colStatus}</th>
              <th className="py-2 pr-4">{t.deliveryList.colAttempts}</th>
            </tr>
          </thead>
          <tbody>
            {deliveries.map((delivery) => (
              <tr key={delivery.delivery_id} className="border-b border-white/[0.06]">
                <td className="py-2 pr-4">
                  <Link
                    to={`/deliveries/${delivery.delivery_id}`}
                    className="font-mono text-xs text-slate-300 underline"
                  >
                    {delivery.delivery_id.slice(0, 8)}
                  </Link>
                </td>
                <td className="py-2 pr-4 text-sm text-slate-300">{delivery.channel}</td>
                <td className="py-2 pr-4 text-sm text-slate-300">{delivery.tier}</td>
                <td className="py-2 pr-4">
                  <span className={`rounded px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[delivery.status]}`}>
                    {delivery.status.replace(/_/g, " ")}
                  </span>
                </td>
                <td className="py-2 pr-4 text-sm text-slate-300">
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
