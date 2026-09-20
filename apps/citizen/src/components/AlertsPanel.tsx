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
import { useTranslation } from "../i18n/LanguageContext";
import type { Translations } from "../i18n/translations";

type View =
  | { kind: "loading" }
  | { kind: "signup" }
  | { kind: "subscribed"; stored: StoredSubscription; rules: AlertSubscriptionRules }
  | { kind: "off" };

export function AlertsPanel() {
  const { t, locale } = useTranslation();
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
      const result = await createCitizenSubscription(rules, pushSubscription, locale);
      const stored: StoredSubscription = {
        subscriptionId: result.subscription_id,
        managementToken: result.management_token,
      };
      saveStoredSubscription(stored);
      setView({ kind: "subscribed", stored, rules });
    } catch (cause) {
      setErrorMessage(describeError(cause, t));
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
      setErrorMessage(describeError(cause, t));
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
      setErrorMessage(describeError(cause, t));
    } finally {
      setSubmitting(false);
    }
  }

  if (view.kind === "loading") {
    return <p className="text-sm text-slate-400">{t.alerts.checking}</p>;
  }

  if (view.kind === "off") {
    return (
      <div className="text-center">
        <h1 className="text-sm font-semibold uppercase tracking-wider text-slate-400">
          {t.alerts.turnedOffTitle}
        </h1>
        <p className="mt-3 text-sm text-slate-500">{t.alerts.turnedOffBody}</p>
        <button
          type="button"
          onClick={() => setView({ kind: "signup" })}
          className="mt-6 rounded-xl border border-white/[0.08] px-4 py-2.5 text-sm font-medium text-slate-300 hover:bg-white/[0.05]"
        >
          {t.alerts.turnBackOn}
        </button>
      </div>
    );
  }

  if (view.kind === "subscribed") {
    return (
      <div>
        <h1 className="text-xl font-bold text-white">{t.alerts.manageHeading}</h1>
        <div className="mt-4 mb-6 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-4 py-2.5 text-sm text-emerald-300">
          {t.alerts.onForDevice}
        </div>
        <AlertRulesForm
          initialRules={view.rules}
          submitting={submitting}
          submitLabel={t.alerts.saveChanges}
          onSubmit={(rules) => void handleUpdate(view.stored, rules)}
        />
        {errorMessage && (
          <p role="alert" className="mt-4 text-sm text-red-400">
            {errorMessage}
          </p>
        )}
        <button
          type="button"
          disabled={submitting}
          onClick={() => void handleTurnOff(view.stored)}
          className="mt-4 w-full rounded-xl border border-red-500/20 px-4 py-2.5 text-sm font-medium text-red-300 hover:bg-red-500/10 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {t.alerts.turnOff}
        </button>
      </div>
    );
  }

  return (
    <div>
      <h1 className="text-xl font-bold text-white">{t.alerts.signupHeading}</h1>
      <p className="mt-1 text-sm text-slate-400">{t.alerts.signupSubtitle}</p>
      <div className="mt-6">
        <AlertRulesForm
          submitting={submitting}
          submitLabel={t.alerts.enableAlerts}
          onSubmit={(rules) => void handleSubscribe(rules)}
        />
      </div>
      {errorMessage && (
        <p role="alert" className="mt-4 text-sm text-red-400">
          {errorMessage}
        </p>
      )}
      <p className="mt-4 text-center text-xs text-slate-500">{t.alerts.permissionNote}</p>
    </div>
  );
}

function describeError(cause: unknown, t: Translations): string {
  if (cause instanceof PushPermissionDeniedError) {
    return t.alerts.errorPermissionDenied;
  }
  if (cause instanceof PushUnsupportedError) {
    return t.alerts.errorUnsupported;
  }
  if (cause instanceof Error) {
    return cause.message;
  }
  return t.alerts.errorUnexpected;
}
