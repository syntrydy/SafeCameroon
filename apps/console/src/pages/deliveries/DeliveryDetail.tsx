import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";

import { ApiError } from "../../api/client";
import { getDelivery, type DeliveryDetail as DeliveryDetailData } from "../../api/deliveries";
import { useAuth } from "../../auth/AuthContext";

export function DeliveryDetail() {
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
      .catch((cause) => setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred."))
      .finally(() => setLoading(false));
  }, [token, id]);

  if (loading) {
    return <p className="text-sm text-slate-500">Loading delivery...</p>;
  }

  if (error && !delivery) {
    return (
      <p role="alert" className="rounded bg-red-50 px-3 py-2 text-sm text-red-700">
        {error}
      </p>
    );
  }

  if (!delivery) {
    return null;
  }

  return (
    <div>
      <h2 className="mb-4 text-base font-semibold text-slate-900">Delivery {delivery.delivery_id.slice(0, 8)}</h2>

      <dl className="mb-6 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="text-slate-500">Channel</dt>
        <dd className="text-slate-900">{delivery.channel}</dd>
        <dt className="text-slate-500">Endpoint</dt>
        <dd className="text-slate-900">{delivery.endpoint_address}</dd>
        <dt className="text-slate-500">Tier</dt>
        <dd className="text-slate-900">{delivery.tier}</dd>
        <dt className="text-slate-500">Status</dt>
        <dd className="text-slate-900">{delivery.status.replace(/_/g, " ")}</dd>
        <dt className="text-slate-500">Attempts</dt>
        <dd className="text-slate-900">
          {delivery.attempt_count} / {delivery.max_attempts}
        </dd>
      </dl>

      <section>
        <h3 className="mb-2 text-sm font-semibold text-slate-900">Attempt history</h3>
        {delivery.attempts.length === 0 ? (
          <p className="text-sm text-slate-500">No attempts have been made yet.</p>
        ) : (
          <table className="w-full border-collapse text-left">
            <thead>
              <tr className="border-b border-slate-200 text-xs uppercase text-slate-500">
                <th className="py-2 pr-4">#</th>
                <th className="py-2 pr-4">Outcome</th>
                <th className="py-2 pr-4">Detail</th>
              </tr>
            </thead>
            <tbody>
              {delivery.attempts.map((attempt) => (
                <tr key={attempt.attempt_number} className="border-b border-slate-100">
                  <td className="py-2 pr-4 text-sm text-slate-700">{attempt.attempt_number}</td>
                  <td className="py-2 pr-4">
                    <span
                      className={`rounded px-2 py-0.5 text-xs font-medium ${
                        attempt.outcome === "SENT" ? "bg-emerald-100 text-emerald-800" : "bg-red-100 text-red-800"
                      }`}
                    >
                      {attempt.outcome}
                    </span>
                  </td>
                  <td className="py-2 pr-4 text-sm text-slate-700">
                    {attempt.outcome === "SENT"
                      ? attempt.provider_message_id
                        ? `Provider message id: ${attempt.provider_message_id}`
                        : "—"
                      : `${attempt.failure_reason ?? "Unknown failure"} (${
                          attempt.retryable ? "retryable" : "permanent"
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
