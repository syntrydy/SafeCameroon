import { useState } from "react";

import { ApiError } from "../../api/client";
import { updateSubscription, type Subscription, type SubscriptionRule } from "../../api/subscriptions";
import { describeRule } from "./describeRule";
import { SubscriptionRuleEditor } from "./SubscriptionRuleEditor";

interface SubscriptionCardProps {
  token: string;
  subscription: Subscription;
  onUpdated: (subscription: Subscription) => void;
}

export function SubscriptionCard({ token, subscription, onUpdated }: SubscriptionCardProps) {
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
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="mb-3 rounded border border-slate-200 p-3">
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
            className="text-xs font-medium text-slate-600 underline"
          >
            Edit
          </button>
        )}
      </div>

      {editing ? (
        <div>
          <SubscriptionRuleEditor rules={rules} onChange={setRules} />
          {error && (
            <p role="alert" className="mt-2 text-sm text-red-700">
              {error}
            </p>
          )}
          <div className="mt-2 flex gap-2">
            <button
              type="button"
              disabled={saving}
              onClick={() => void handleSave()}
              className="rounded bg-slate-900 px-3 py-1 text-xs font-medium text-white disabled:opacity-50"
            >
              {saving ? "Saving..." : "Save changes"}
            </button>
            <button
              type="button"
              onClick={() => setEditing(false)}
              className="rounded border border-slate-300 px-3 py-1 text-xs text-slate-700"
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <ul className="text-sm text-slate-700">
          {subscription.rules.map((rule, index) => (
            <li key={index}>{describeRule(rule)}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
