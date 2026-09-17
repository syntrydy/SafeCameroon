import { useState, type FormEvent } from "react";

import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";
import { getConsumer, registerConsumer, type Consumer, type ConsumerType } from "../../api/consumers";
import {
  createSubscription,
  getDeliveryPreference,
  listSubscriptionsForConsumer,
  type DeliveryPreference,
  type Subscription,
  type SubscriptionRule,
} from "../../api/subscriptions";
import { DeliveryPreferenceForm } from "./DeliveryPreferenceForm";
import { SubscriptionCard } from "./SubscriptionCard";
import { SubscriptionRuleEditor } from "./SubscriptionRuleEditor";

function CreateSubscriptionForm({
  token,
  consumerId,
  onCreated,
}: {
  token: string;
  consumerId: string;
  onCreated: (subscription: Subscription) => void;
}) {
  const { t } = useTranslation();
  const [adding, setAdding] = useState(false);
  const [rules, setRules] = useState<SubscriptionRule[]>([{ rule: "INCIDENT_TYPE", values: [] }]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!adding) {
    return (
      <button type="button" onClick={() => setAdding(true)} className="text-sm font-medium text-slate-700 underline">
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
    <div className="rounded border border-slate-200 p-3">
      <SubscriptionRuleEditor rules={rules} onChange={setRules} />
      {error && (
        <p role="alert" className="mt-2 text-sm text-red-700">
          {error}
        </p>
      )}
      <div className="mt-2 flex gap-2">
        <button
          type="button"
          disabled={submitting}
          onClick={() => void handleCreate()}
          className="rounded bg-slate-900 px-3 py-1 text-xs font-medium text-white disabled:opacity-50"
        >
          {submitting ? t.subscriptions.creating : t.subscriptions.createSubscription}
        </button>
        <button
          type="button"
          onClick={() => setAdding(false)}
          className="rounded border border-slate-300 px-3 py-1 text-xs text-slate-700"
        >
          {t.common.cancel}
        </button>
      </div>
    </div>
  );
}

export function Subscriptions() {
  const { t } = useTranslation();
  const [consumerIdInput, setConsumerIdInput] = useState("");
  const [newConsumerName, setNewConsumerName] = useState("");
  const [newConsumerType, setNewConsumerType] = useState<ConsumerType>("ORGANIZATION");
  const [lookupError, setLookupError] = useState<string | null>(null);

  const [consumer, setConsumer] = useState<Consumer | null>(null);
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [deliveryPreference, setDeliveryPreference] = useState<DeliveryPreference | null>(null);
  const [loadingData, setLoadingData] = useState(false);
  const [dataError, setDataError] = useState<string | null>(null);

  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>.
  const token = session!.token;

  async function loadConsumerData(consumerId: string) {
    setLoadingData(true);
    setDataError(null);
    try {
      setSubscriptions(await listSubscriptionsForConsumer(token, consumerId));
    } catch (cause) {
      setDataError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    }
    try {
      setDeliveryPreference(await getDeliveryPreference(token, consumerId));
    } catch (cause) {
      if (cause instanceof ApiError && cause.code === "DELIVERY_PREFERENCE_NOT_FOUND") {
        setDeliveryPreference(null);
      } else {
        setDataError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
      }
    }
    setLoadingData(false);
  }

  async function handleLookup(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setLookupError(null);
    try {
      const found = await getConsumer(token, consumerIdInput.trim());
      setConsumer(found);
      await loadConsumerData(found.consumer_id);
    } catch (cause) {
      setLookupError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    }
  }

  async function handleCreateConsumer(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setLookupError(null);
    try {
      const created = await registerConsumer(token, newConsumerName, newConsumerType);
      setConsumer(created);
      setNewConsumerName("");
      await loadConsumerData(created.consumer_id);
    } catch (cause) {
      setLookupError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    }
  }

  function switchConsumer() {
    setConsumer(null);
    setSubscriptions([]);
    setDeliveryPreference(null);
    setConsumerIdInput("");
  }

  return (
    <div>
      <h2 className="mb-4 text-base font-semibold text-slate-900">{t.subscriptions.heading}</h2>

      {!consumer ? (
        <div className="grid gap-6 sm:grid-cols-2">
          <form onSubmit={(event) => void handleLookup(event)} className="rounded border border-slate-200 p-4">
            <h3 className="mb-2 text-sm font-semibold text-slate-900">{t.subscriptions.lookupHeading}</h3>
            <label className="mb-2 block text-sm">
              <span className="mb-1 block text-slate-700">{t.subscriptions.consumerId}</span>
              <input
                required
                value={consumerIdInput}
                onChange={(event) => setConsumerIdInput(event.target.value)}
                className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
              />
            </label>
            <button type="submit" className="rounded bg-slate-900 px-3 py-1.5 text-sm font-medium text-white">
              {t.subscriptions.load}
            </button>
          </form>

          <form
            onSubmit={(event) => void handleCreateConsumer(event)}
            className="rounded border border-slate-200 p-4"
          >
            <h3 className="mb-2 text-sm font-semibold text-slate-900">{t.subscriptions.registerHeading}</h3>
            <label className="mb-2 block text-sm">
              <span className="mb-1 block text-slate-700">{t.subscriptions.name}</span>
              <input
                required
                value={newConsumerName}
                onChange={(event) => setNewConsumerName(event.target.value)}
                className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
              />
            </label>
            <label className="mb-2 block text-sm">
              <span className="mb-1 block text-slate-700">{t.subscriptions.type}</span>
              <select
                value={newConsumerType}
                onChange={(event) => setNewConsumerType(event.target.value as ConsumerType)}
                className="w-full rounded border border-slate-300 px-2 py-1 text-sm"
              >
                <option value="ORGANIZATION">{t.subscriptions.typeOrganization}</option>
                <option value="CITIZEN">{t.subscriptions.typeCitizen}</option>
              </select>
            </label>
            <button type="submit" className="rounded bg-slate-900 px-3 py-1.5 text-sm font-medium text-white">
              {t.subscriptions.createConsumer}
            </button>
          </form>
        </div>
      ) : (
        <div>
          <div className="mb-4 flex items-center justify-between rounded border border-slate-200 p-3">
            <div>
              <p className="text-sm font-medium text-slate-900">{consumer.name}</p>
              <p className="text-xs text-slate-500">
                {consumer.consumer_type} &middot; {consumer.consumer_id}
              </p>
            </div>
            <button type="button" onClick={switchConsumer} className="text-xs font-medium text-slate-600 underline">
              {t.subscriptions.switchConsumer}
            </button>
          </div>

          {dataError && (
            <p role="alert" className="mb-4 rounded bg-red-50 px-3 py-2 text-sm text-red-700">
              {dataError}
            </p>
          )}

          <section className="mb-6">
            <h3 className="mb-2 text-sm font-semibold text-slate-900">{t.subscriptions.subscriptionsHeading}</h3>
            {loadingData ? (
              <p className="text-sm text-slate-500">{t.subscriptions.loading}</p>
            ) : (
              subscriptions.map((subscription) => (
                <SubscriptionCard
                  key={subscription.subscription_id}
                  token={token}
                  subscription={subscription}
                  onUpdated={(updated) =>
                    setSubscriptions((current) =>
                      current.map((s) => (s.subscription_id === updated.subscription_id ? updated : s)),
                    )
                  }
                />
              ))
            )}
            <CreateSubscriptionForm
              token={token}
              consumerId={consumer.consumer_id}
              onCreated={(created) => setSubscriptions((current) => [...current, created])}
            />
          </section>

          <section>
            <h3 className="mb-2 text-sm font-semibold text-slate-900">{t.subscriptions.deliveryPreferenceHeading}</h3>
            {!loadingData && !deliveryPreference && (
              <p className="mb-2 text-sm text-slate-500">{t.subscriptions.noDeliveryPreference}</p>
            )}
            {!loadingData && (
              <DeliveryPreferenceForm
                token={token}
                consumerId={consumer.consumer_id}
                initial={deliveryPreference}
                onSaved={setDeliveryPreference}
              />
            )}
          </section>
        </div>
      )}

      {lookupError && (
        <p role="alert" className="mt-4 rounded bg-red-50 px-3 py-2 text-sm text-red-700">
          {lookupError}
        </p>
      )}
    </div>
  );
}
