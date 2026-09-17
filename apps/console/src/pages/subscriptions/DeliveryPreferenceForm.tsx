import { useState, type FormEvent } from "react";

import { ApiError } from "../../api/client";
import { useTranslation } from "../../i18n/LanguageContext";
import {
  setDeliveryPreference,
  type ChannelEndpoint,
  type ChannelType,
  type DeliveryPreference,
  type DeliveryStrategy,
} from "../../api/subscriptions";

const CHANNELS: ChannelType[] = ["WHATSAPP", "SMS", "EMAIL"];

interface DeliveryPreferenceFormProps {
  token: string;
  consumerId: string;
  initial: DeliveryPreference | null;
  onSaved: (preference: DeliveryPreference) => void;
}

export function DeliveryPreferenceForm({ token, consumerId, initial, onSaved }: DeliveryPreferenceFormProps) {
  const { t } = useTranslation();
  const strategies: { value: DeliveryStrategy; label: string }[] = [
    { value: "ALL", label: t.deliveryPreferenceForm.strategyAll },
    { value: "PRIMARY_FALLBACK", label: t.deliveryPreferenceForm.strategyPrimaryFallback },
    { value: "PRIORITY_LIST", label: t.deliveryPreferenceForm.strategyPriorityList },
  ];
  const [strategy, setStrategy] = useState<DeliveryStrategy>(initial?.strategy ?? "ALL");
  const [channels, setChannels] = useState<ChannelEndpoint[]>(
    initial?.channels ?? [{ channel: "WHATSAPP", address: "" }],
  );
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function updateChannel(index: number, endpoint: ChannelEndpoint) {
    setChannels((current) => current.map((existing, i) => (i === index ? endpoint : existing)));
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const saved = await setDeliveryPreference(token, consumerId, strategy, channels);
      onSaved(saved);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit}>
      <label className="mb-3 block text-sm">
        <span className="mb-1 block font-medium text-slate-300">{t.deliveryPreferenceForm.strategy}</span>
        <select
          value={strategy}
          onChange={(event) => setStrategy(event.target.value as DeliveryStrategy)}
          className="rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
        >
          {strategies.map((option) => (
            <option key={option.value} value={option.value} className="bg-slate-900 text-white">
              {option.label}
            </option>
          ))}
        </select>
      </label>

      {channels.map((endpoint, index) => (
        <div key={index} className="mb-2 flex items-center gap-2">
          <select
            aria-label={t.deliveryPreferenceForm.channelLabel(index + 1)}
            value={endpoint.channel}
            onChange={(event) => updateChannel(index, { ...endpoint, channel: event.target.value as ChannelType })}
            className="rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          >
            {CHANNELS.map((channel) => (
              <option key={channel} value={channel} className="bg-slate-900 text-white">
                {channel}
              </option>
            ))}
          </select>
          <input
            aria-label={t.deliveryPreferenceForm.addressLabel(index + 1)}
            value={endpoint.address}
            onChange={(event) => updateChannel(index, { ...endpoint, address: event.target.value })}
            placeholder={t.deliveryPreferenceForm.addressPlaceholder}
            className="rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
          <button
            type="button"
            onClick={() => setChannels((current) => current.filter((_, i) => i !== index))}
            className="text-xs font-medium text-red-300 underline"
          >
            {t.common.remove}
          </button>
        </div>
      ))}

      <button
        type="button"
        onClick={() => setChannels((current) => [...current, { channel: "WHATSAPP", address: "" }])}
        className="mb-3 block text-sm font-medium text-slate-300 underline"
      >
        {t.deliveryPreferenceForm.addChannel}
      </button>

      {error && (
        <p role="alert" className="mb-3 text-sm text-red-300">
          {error}
        </p>
      )}

      <button
        type="submit"
        disabled={submitting}
        className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {submitting ? t.deliveryPreferenceForm.saving : t.deliveryPreferenceForm.saveDeliveryPreference}
      </button>
    </form>
  );
}
