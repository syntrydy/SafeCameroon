import { useEffect, useState } from "preact/hooks";

import {
  cancelCitizenSubscription,
  createCitizenSubscription,
  getCitizenSubscription,
  updateCitizenSubscription,
  type AlertSubscriptionRules,
} from "../api/subscriptions";
import {
  PushPermissionDeniedError,
  PushUnsupportedError,
  subscribeToPush,
  unsubscribeFromPush,
} from "../push/subscribe";
import {
  clearStoredSubscription,
  loadStoredSubscription,
  saveStoredSubscription,
  type StoredSubscription,
} from "../offline/subscriptionStorage";
import { AlertRulesForm } from "./AlertRulesForm";

type View =
  | { kind: "loading" }
  | { kind: "signup" }
  | { kind: "subscribed"; stored: StoredSubscription; rules: AlertSubscriptionRules }
  | { kind: "off" };

export function AlertsPanel() {
  const [view, setView] = useState<View>({ kind: "loading" });
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    const stored = loadStoredSubscription();
    if (!stored) {
      setView({ kind: "signup" });
      return;
    }
    getCitizenSubscription(stored.subscriptionId, stored.managementToken)
      .then((result) => {
        setView({
          kind: "subscribed",
          stored,
          rules: {
            incidentTypes: result.incident_types,
            minimumSeverity: result.minimum_severity,
            geography: result.geography,
          },
        });
      })
      .catch(() => {
        // The subscription was cancelled elsewhere, or the token no longer
        // matches -- either way, this device has nothing usable left.
        clearStoredSubscription();
        setView({ kind: "signup" });
      });
  }, []);

  async function handleSubscribe(rules: AlertSubscriptionRules) {
    setSubmitting(true);
    setErrorMessage(null);
    try {
      const pushSubscription = await subscribeToPush();
      const result = await createCitizenSubscription(rules, pushSubscription);
      const stored: StoredSubscription = {
        subscriptionId: result.subscription_id,
        managementToken: result.management_token,
      };
      saveStoredSubscription(stored);
      setView({ kind: "subscribed", stored, rules });
    } catch (cause) {
      setErrorMessage(describeError(cause));
    } finally {
      setSubmitting(false);
    }
  }

  async function handleUpdate(stored: StoredSubscription, rules: AlertSubscriptionRules) {
    setSubmitting(true);
    setErrorMessage(null);
    try {
      await updateCitizenSubscription(stored.subscriptionId, stored.managementToken, rules);
      setView({ kind: "subscribed", stored, rules });
    } catch (cause) {
      setErrorMessage(describeError(cause));
    } finally {
      setSubmitting(false);
    }
  }

  async function handleTurnOff(stored: StoredSubscription) {
    setSubmitting(true);
    setErrorMessage(null);
    try {
      await cancelCitizenSubscription(stored.subscriptionId, stored.managementToken);
      await unsubscribeFromPush();
      clearStoredSubscription();
      setView({ kind: "off" });
    } catch (cause) {
      setErrorMessage(describeError(cause));
    } finally {
      setSubmitting(false);
    }
  }

  if (view.kind === "loading") {
    return <p className="text-sm text-slate-500">Checking your alert settings...</p>;
  }

  if (view.kind === "off") {
    return (
      <div className="text-center">
        <p className="text-sm font-semibold uppercase tracking-wider text-slate-500">
          Alerts turned off
        </p>
        <p className="mt-3 text-sm text-slate-600">
          You will no longer receive alerts on this device.
        </p>
        <button
          type="button"
          onClick={() => setView({ kind: "signup" })}
          className="mt-6 rounded-xl border border-slate-300 px-4 py-2.5 text-sm font-medium text-slate-700 hover:bg-slate-50"
        >
          Turn alerts back on
        </button>
      </div>
    );
  }

  if (view.kind === "subscribed") {
    return (
      <div>
        <div className="mb-6 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-2.5 text-sm text-emerald-800">
          Alerts are on for this device.
        </div>
        <AlertRulesForm
          initialRules={view.rules}
          submitting={submitting}
          submitLabel="Save changes"
          onSubmit={(rules) => void handleUpdate(view.stored, rules)}
        />
        {errorMessage && (
          <p role="alert" className="mt-4 text-sm text-red-600">
            {errorMessage}
          </p>
        )}
        <button
          type="button"
          disabled={submitting}
          onClick={() => void handleTurnOff(view.stored)}
          className="mt-4 w-full rounded-xl border border-red-200 px-4 py-2.5 text-sm font-medium text-red-700 hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-60"
        >
          Turn off alerts
        </button>
      </div>
    );
  }

  return (
    <div>
      <h2 className="text-xl font-semibold text-slate-900">Get missing-child alerts</h2>
      <p className="mt-1 text-sm text-slate-500">
        Choose what you want to hear about. You can change this anytime.
      </p>
      <div className="mt-6">
        <AlertRulesForm
          submitting={submitting}
          submitLabel="Enable alerts"
          onSubmit={(rules) => void handleSubscribe(rules)}
        />
      </div>
      {errorMessage && (
        <p role="alert" className="mt-4 text-sm text-red-600">
          {errorMessage}
        </p>
      )}
      <p className="mt-4 text-center text-xs text-slate-400">
        Your browser will ask permission to send notifications. No account or personal
        information is required.
      </p>
    </div>
  );
}

function describeError(cause: unknown): string {
  if (cause instanceof PushPermissionDeniedError) {
    return "Notifications were not allowed. You can enable them in your browser's site settings and try again.";
  }
  if (cause instanceof PushUnsupportedError) {
    return "This browser does not support push notifications.";
  }
  if (cause instanceof Error) {
    return cause.message;
  }
  return "An unexpected error occurred.";
}
