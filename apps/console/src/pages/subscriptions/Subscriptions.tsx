import { useCallback, useEffect, useState, type FormEvent } from "react";

import { ApiError } from "../../api/client";
import { useAuth } from "../../auth/AuthContext";
import { useTranslation } from "../../i18n/LanguageContext";
import { getConsumer, registerConsumer, type Consumer, type ConsumerType } from "../../api/consumers";
import {
  getDeliveryPreference,
  listAllSubscriptions,
  listSubscriptionsForConsumer,
  type DeliveryPreference,
  type Subscription,
} from "../../api/subscriptions";
import { AllSubscriberRow } from "./AllSubscriberRow";
import { CreateSubscriptionForm } from "./CreateSubscriptionForm";
import { DeliveryPreferenceForm } from "./DeliveryPreferenceForm";
import { SubscriptionCard } from "./SubscriptionCard";

const ALL_SUBSCRIPTIONS_PAGE_SIZE = 50;

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

  const [allSubscriptions, setAllSubscriptions] = useState<Subscription[]>([]);
  const [allSubscriptionsLoading, setAllSubscriptionsLoading] = useState(false);
  const [allSubscriptionsError, setAllSubscriptionsError] = useState<string | null>(null);
  const [hasMoreSubscriptions, setHasMoreSubscriptions] = useState(true);

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

  async function viewConsumer(consumerId: string) {
    setLookupError(null);
    try {
      const found = await getConsumer(token, consumerId);
      setConsumer(found);
      await loadConsumerData(found.consumer_id);
    } catch (cause) {
      setLookupError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    }
  }

  async function handleLookup(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await viewConsumer(consumerIdInput.trim());
  }

  const loadAllSubscriptions = useCallback(
    async (offset: number) => {
      setAllSubscriptionsLoading(true);
      setAllSubscriptionsError(null);
      try {
        const page = await listAllSubscriptions(token, {
          limit: ALL_SUBSCRIPTIONS_PAGE_SIZE,
          offset,
        });
        setAllSubscriptions((current) => (offset === 0 ? page : [...current, ...page]));
        setHasMoreSubscriptions(page.length === ALL_SUBSCRIPTIONS_PAGE_SIZE);
      } catch (cause) {
        setAllSubscriptionsError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
      } finally {
        setAllSubscriptionsLoading(false);
      }
    },
    [token, t],
  );

  useEffect(() => {
    void loadAllSubscriptions(0);
  }, [loadAllSubscriptions]);

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
      <h1 className="mb-4 text-base font-semibold text-white">{t.subscriptions.heading}</h1>

      {!consumer ? (
        <div className="grid gap-6 sm:grid-cols-2">
          <form onSubmit={(event) => void handleLookup(event)} className="rounded border border-white/[0.08] p-4">
            <h2 className="mb-2 text-sm font-semibold text-white">{t.subscriptions.lookupHeading}</h2>
            <label className="mb-2 block text-sm">
              <span className="mb-1 block text-slate-300">{t.subscriptions.consumerId}</span>
              <input
                required
                value={consumerIdInput}
                onChange={(event) => setConsumerIdInput(event.target.value)}
                className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
              />
            </label>
            <button type="submit" className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600">
              {t.subscriptions.load}
            </button>
          </form>

          <form
            onSubmit={(event) => void handleCreateConsumer(event)}
            className="rounded border border-white/[0.08] p-4"
          >
            <h2 className="mb-2 text-sm font-semibold text-white">{t.subscriptions.registerHeading}</h2>
            <label className="mb-2 block text-sm">
              <span className="mb-1 block text-slate-300">{t.subscriptions.name}</span>
              <input
                required
                value={newConsumerName}
                onChange={(event) => setNewConsumerName(event.target.value)}
                className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
              />
            </label>
            <label className="mb-2 block text-sm">
              <span className="mb-1 block text-slate-300">{t.subscriptions.type}</span>
              <select
                value={newConsumerType}
                onChange={(event) => setNewConsumerType(event.target.value as ConsumerType)}
                className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
              >
                <option value="ORGANIZATION" className="bg-slate-900 text-white">{t.subscriptions.typeOrganization}</option>
                <option value="CITIZEN" className="bg-slate-900 text-white">{t.subscriptions.typeCitizen}</option>
              </select>
            </label>
            <button type="submit" className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600">
              {t.subscriptions.createConsumer}
            </button>
          </form>
        </div>
      ) : null}

      {!consumer && (
        <section className="mt-6 rounded border border-white/[0.08] p-4">
          <h2 className="mb-2 text-sm font-semibold text-white">
            {t.subscriptions.allSubscriptionsHeading}
          </h2>
          {allSubscriptionsError && (
            <p role="alert" className="mb-2 rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
              {allSubscriptionsError}
            </p>
          )}
          {allSubscriptions.length === 0 && !allSubscriptionsLoading && !allSubscriptionsError && (
            <p className="text-sm text-slate-500">{t.subscriptions.noSubscriptionsAtAll}</p>
          )}
          {allSubscriptions.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full border-collapse text-left">
                <thead>
                  <tr className="border-b border-white/[0.08] text-xs uppercase text-slate-500">
                    <th className="py-2 pr-4">{t.subscriptions.colSubscriber}</th>
                    <th className="py-2 pr-4">{t.subscriptions.colRules}</th>
                    <th className="py-2 pr-4">{t.subscriptions.colVersion}</th>
                    <th className="py-2 pr-4">{t.subscriptions.colCreated}</th>
                    <th className="py-2 pr-4">{t.subscriptions.colActions}</th>
                  </tr>
                </thead>
                <tbody>
                  {allSubscriptions.map((subscription) => (
                    <AllSubscriberRow
                      key={subscription.subscription_id}
                      token={token}
                      subscription={subscription}
                      onViewSubscriber={(consumerId) => void viewConsumer(consumerId)}
                      onUpdated={(updated) =>
                        setAllSubscriptions((current) =>
                          current.map((s) => (s.subscription_id === updated.subscription_id ? updated : s)),
                        )
                      }
                    />
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {allSubscriptionsLoading && <p className="text-sm text-slate-500">{t.subscriptions.loading}</p>}
          {hasMoreSubscriptions && !allSubscriptionsLoading && allSubscriptions.length > 0 && (
            <button
              type="button"
              onClick={() => void loadAllSubscriptions(allSubscriptions.length)}
              className="mt-2 rounded-lg border border-white/[0.08] px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-white/[0.05]"
            >
              {t.subscriptions.loadMore}
            </button>
          )}
        </section>
      )}

      {consumer && (
        <div>
          <div className="mb-4 flex items-center justify-between rounded border border-white/[0.08] p-3">
            <div>
              <p className="text-sm font-medium text-white">{consumer.name}</p>
              <p className="text-xs text-slate-500">
                {consumer.consumer_type} &middot; {consumer.consumer_id}
              </p>
            </div>
            <button type="button" onClick={switchConsumer} className="text-xs font-medium text-slate-400 underline">
              {t.subscriptions.switchConsumer}
            </button>
          </div>

          {dataError && (
            <p role="alert" className="mb-4 rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
              {dataError}
            </p>
          )}

          <section className="mb-6">
            <h2 className="mb-2 text-sm font-semibold text-white">{t.subscriptions.subscriptionsHeading}</h2>
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
            <h2 className="mb-2 text-sm font-semibold text-white">{t.subscriptions.deliveryPreferenceHeading}</h2>
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
        <p role="alert" className="mt-4 rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
          {lookupError}
        </p>
      )}
    </div>
  );
}
