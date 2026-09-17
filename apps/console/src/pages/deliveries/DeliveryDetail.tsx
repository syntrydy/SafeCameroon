import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";

import { ApiError } from "../../api/client";
import { getDelivery, type DeliveryDetail as DeliveryDetailData } from "../../api/deliveries";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";

export function DeliveryDetail() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>.
  const token = session!.token;

  const [delivery, setDelivery] = useState<DeliveryDetailData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    setError(null);
    getDelivery(token, id)
      .then(setDelivery)
      .catch((cause) => setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError))
      .finally(() => setLoading(false));
  }, [token, id, t]);

  if (loading) {
    return <p className="text-sm text-slate-500">{t.deliveryDetail.loading}</p>;
  }

  if (error && !delivery) {
    return (
      <p role="alert" className="rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
        {error}
      </p>
    );
  }

  if (!delivery) {
    return null;
  }

  return (
    <div>
      <h2 className="mb-4 text-base font-semibold text-white">
        {t.deliveryDetail.heading(delivery.delivery_id.slice(0, 8))}
      </h2>

      <dl className="mb-6 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="text-slate-500">{t.deliveryDetail.channel}</dt>
        <dd className="text-white">{delivery.channel}</dd>
        <dt className="text-slate-500">{t.deliveryDetail.endpoint}</dt>
        <dd className="text-white">{delivery.endpoint_address}</dd>
        <dt className="text-slate-500">{t.deliveryDetail.tier}</dt>
        <dd className="text-white">{delivery.tier}</dd>
        <dt className="text-slate-500">{t.deliveryDetail.status}</dt>
        <dd className="text-white">{delivery.status.replace(/_/g, " ")}</dd>
        <dt className="text-slate-500">{t.deliveryDetail.attempts}</dt>
        <dd className="text-white">
          {delivery.attempt_count} / {delivery.max_attempts}
        </dd>
      </dl>

      <section>
        <h3 className="mb-2 text-sm font-semibold text-white">{t.deliveryDetail.attemptHistory}</h3>
        {delivery.attempts.length === 0 ? (
          <p className="text-sm text-slate-500">{t.deliveryDetail.noAttempts}</p>
        ) : (
          <table className="w-full border-collapse text-left">
            <thead>
              <tr className="border-b border-white/[0.08] text-xs uppercase text-slate-500">
                <th className="py-2 pr-4">{t.deliveryDetail.colNumber}</th>
                <th className="py-2 pr-4">{t.deliveryDetail.colOutcome}</th>
                <th className="py-2 pr-4">{t.deliveryDetail.colDetail}</th>
              </tr>
            </thead>
            <tbody>
              {delivery.attempts.map((attempt) => (
                <tr key={attempt.attempt_number} className="border-b border-white/[0.06]">
                  <td className="py-2 pr-4 text-sm text-slate-300">{attempt.attempt_number}</td>
                  <td className="py-2 pr-4">
                    <span
                      className={`rounded px-2 py-0.5 text-xs font-medium ${
                        attempt.outcome === "SENT" ? "bg-emerald-500/10 text-emerald-300" : "bg-red-500/10 text-red-300"
                      }`}
                    >
                      {attempt.outcome}
                    </span>
                  </td>
                  <td className="py-2 pr-4 text-sm text-slate-300">
                    {attempt.outcome === "SENT"
                      ? attempt.provider_message_id
                        ? t.deliveryDetail.providerMessageId(attempt.provider_message_id)
                        : "—"
                      : `${attempt.failure_reason ?? t.deliveryDetail.unknownFailure} (${
                          attempt.retryable ? t.deliveryDetail.retryable : t.deliveryDetail.permanent
                        })`}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
