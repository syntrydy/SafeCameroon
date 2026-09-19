import { useState } from "react";

import { ApiError } from "../../api/client";
import { updateSubscription, type Subscription, type SubscriptionRule } from "../../api/subscriptions";
import { useTranslation } from "../../i18n/LanguageContext";
import { describeRule } from "./describeRule";
import { SubscriptionRuleEditor } from "./SubscriptionRuleEditor";

interface AllSubscriberRowProps {
  token: string;
  subscription: Subscription;
  onUpdated: (subscription: Subscription) => void;
  onViewSubscriber: (consumerId: string) => void;
}

// One row in the "All subscribers" table (Subscriptions.tsx) -- a table
// row, unlike SubscriptionCard's bordered-box layout, so it needs its own
// edit/save state rather than reusing that component directly.
export function AllSubscriberRow({ token, subscription, onUpdated, onViewSubscriber }: AllSubscriberRowProps) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [rules, setRules] = useState<SubscriptionRule[]>(subscription.rules);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const updated = await updateSubscription(token, subscription.subscription_id, rules);
      onUpdated(updated);
      setEditing(false);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setSaving(false);
    }
  }

  return (
    <tr className="border-b border-white/[0.06] align-top">
      <td className="py-3 pr-4">
        <span className="font-mono text-xs text-slate-500">{subscription.consumer_id.slice(0, 8)}</span>
        <button
          type="button"
          onClick={() => onViewSubscriber(subscription.consumer_id)}
          className="mt-1 block text-xs font-medium text-emerald-400 underline"
        >
          {t.subscriptions.viewConsumer}
        </button>
      </td>
      <td className="py-3 pr-4">
        {editing ? (
          <div>
            <SubscriptionRuleEditor rules={rules} onChange={setRules} />
            {error && (
              <p role="alert" className="mt-2 text-sm text-red-300">
                {error}
              </p>
            )}
          </div>
        ) : (
          <ul className="text-sm text-slate-300">
            {subscription.rules.map((rule, index) => (
              <li key={index}>{describeRule(rule, t)}</li>
            ))}
          </ul>
        )}
      </td>
      <td className="py-3 pr-4 text-sm text-slate-400">v{subscription.version}</td>
      <td className="py-3 pr-4 text-sm text-slate-400">
        {subscription.created_at ? new Date(subscription.created_at).toLocaleString() : "—"}
      </td>
      <td className="py-3 pr-4">
        {editing ? (
          <div className="flex gap-2">
            <button
              type="button"
              disabled={saving}
              onClick={() => void handleSave()}
              className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1 text-xs font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {saving ? t.subscriptionCard.saving : t.subscriptionCard.saveChanges}
            </button>
            <button
              type="button"
              onClick={() => {
                setRules(subscription.rules);
                setEditing(false);
              }}
              className="rounded-lg border border-white/[0.08] px-3 py-1 text-xs text-slate-300 hover:bg-white/[0.05]"
            >
              {t.subscriptionCard.cancel}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => {
              setRules(subscription.rules);
              setEditing(true);
            }}
            className="text-xs font-medium text-slate-400 underline"
          >
            {t.subscriptionCard.edit}
          </button>
        )}
      </td>
    </tr>
  );
}
