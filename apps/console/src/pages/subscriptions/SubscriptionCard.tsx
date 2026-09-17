import { useState } from "react";

import { ApiError } from "../../api/client";
import { updateSubscription, type Subscription, type SubscriptionRule } from "../../api/subscriptions";
import { useTranslation } from "../../i18n/LanguageContext";
import { describeRule } from "./describeRule";
import { SubscriptionRuleEditor } from "./SubscriptionRuleEditor";

interface SubscriptionCardProps {
  token: string;
  subscription: Subscription;
  onUpdated: (subscription: Subscription) => void;
}

export function SubscriptionCard({ token, subscription, onUpdated }: SubscriptionCardProps) {
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
    <div className="mb-3 rounded border border-white/[0.08] p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="font-mono text-xs text-slate-500">
          {subscription.subscription_id.slice(0, 8)} (v{subscription.version})
        </span>
        {!editing && (
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
      </div>

      {editing ? (
        <div>
          <SubscriptionRuleEditor rules={rules} onChange={setRules} />
          {error && (
            <p role="alert" className="mt-2 text-sm text-red-300">
              {error}
            </p>
          )}
          <div className="mt-2 flex gap-2">
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
              onClick={() => setEditing(false)}
              className="rounded-lg border border-white/[0.08] px-3 py-1 text-xs text-slate-300 hover:bg-white/[0.05]"
            >
              {t.subscriptionCard.cancel}
            </button>
          </div>
        </div>
      ) : (
        <ul className="text-sm text-slate-300">
          {subscription.rules.map((rule, index) => (
            <li key={index}>{describeRule(rule, t)}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
