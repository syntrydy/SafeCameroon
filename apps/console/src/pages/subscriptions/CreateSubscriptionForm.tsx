import { useState } from "react";

import { ApiError } from "../../api/client";
import { createSubscription, type Subscription, type SubscriptionRule } from "../../api/subscriptions";
import { useTranslation } from "../../i18n/LanguageContext";
import { SubscriptionRuleEditor } from "./SubscriptionRuleEditor";

interface CreateSubscriptionFormProps {
  token: string;
  consumerId: string;
  onCreated: (subscription: Subscription) => void;
}

export function CreateSubscriptionForm({ token, consumerId, onCreated }: CreateSubscriptionFormProps) {
  const { t } = useTranslation();
  const [adding, setAdding] = useState(false);
  const [rules, setRules] = useState<SubscriptionRule[]>([{ rule: "INCIDENT_TYPE", values: [] }]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!adding) {
    return (
      <button type="button" onClick={() => setAdding(true)} className="text-sm font-medium text-slate-300 underline">
        {t.subscriptions.addSubscription}
      </button>
    );
  }

  async function handleCreate() {
    setSubmitting(true);
    setError(null);
    try {
      const created = await createSubscription(token, consumerId, rules);
      onCreated(created);
      setAdding(false);
      setRules([{ rule: "INCIDENT_TYPE", values: [] }]);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="rounded border border-white/[0.08] p-3">
      <SubscriptionRuleEditor rules={rules} onChange={setRules} />
      {error && (
        <p role="alert" className="mt-2 text-sm text-red-300">
          {error}
        </p>
      )}
      <div className="mt-2 flex gap-2">
        <button
          type="button"
          disabled={submitting}
          onClick={() => void handleCreate()}
          className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1 text-xs font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {submitting ? t.subscriptions.creating : t.subscriptions.createSubscription}
        </button>
        <button
          type="button"
          onClick={() => setAdding(false)}
          className="rounded-lg border border-white/[0.08] px-3 py-1 text-xs text-slate-300 hover:bg-white/[0.05]"
        >
          {t.common.cancel}
        </button>
      </div>
    </div>
  );
}
