import { useState } from "preact/hooks";

import type { AlertSubscriptionRules, IncidentType, Severity } from "../api/subscriptions";
import { CAMEROON_TOWNS } from "../domain/cameroonTowns";

const INCIDENT_TYPE_OPTIONS: { value: IncidentType; label: string }[] = [
  { value: "MISSING_CHILD", label: "Missing child" },
  { value: "OTHER_PROTECTION_INCIDENT", label: "Other protection incident" },
];

const SEVERITY_OPTIONS: { value: Severity; label: string }[] = [
  { value: "LOW", label: "Low and above" },
  { value: "MEDIUM", label: "Medium and above" },
  { value: "HIGH", label: "High and above" },
  { value: "CRITICAL", label: "Critical only" },
];

interface AlertRulesFormProps {
  initialRules?: AlertSubscriptionRules;
  submitting: boolean;
  submitLabel: string;
  onSubmit: (rules: AlertSubscriptionRules) => void;
}

const DEFAULT_RULES: AlertSubscriptionRules = {
  incidentTypes: ["MISSING_CHILD"],
  minimumSeverity: "HIGH",
  geography: "",
};

export function AlertRulesForm({
  initialRules,
  submitting,
  submitLabel,
  onSubmit,
}: AlertRulesFormProps) {
  const [incidentTypes, setIncidentTypes] = useState<IncidentType[]>(
    initialRules?.incidentTypes ?? DEFAULT_RULES.incidentTypes,
  );
  const [minimumSeverity, setMinimumSeverity] = useState<Severity>(
    initialRules?.minimumSeverity ?? DEFAULT_RULES.minimumSeverity,
  );
  const [geography, setGeography] = useState(initialRules?.geography ?? DEFAULT_RULES.geography);
  const [touched, setTouched] = useState(false);

  const geographyIsBlank = geography.trim().length === 0;
  const noIncidentTypes = incidentTypes.length === 0;

  function toggleIncidentType(value: IncidentType) {
    setIncidentTypes((current) =>
      current.includes(value)
        ? current.filter((item) => item !== value)
        : [...current, value],
    );
  }

  function handleSubmit(event: Event) {
    event.preventDefault();
    setTouched(true);
    if (geographyIsBlank || noIncidentTypes) {
      return;
    }
    onSubmit({ incidentTypes, minimumSeverity, geography: geography.trim() });
  }

  return (
    <form onSubmit={handleSubmit} noValidate>
      <fieldset>
        <legend className="text-sm font-medium text-slate-300">Alert type</legend>
        <div className="mt-2.5 space-y-2">
          {INCIDENT_TYPE_OPTIONS.map((option) => (
            <label
              key={option.value}
              className="flex items-center gap-2.5 rounded-xl border border-white/[0.06] bg-white/[0.03] px-3.5 py-2.5 text-sm text-slate-300"
            >
              <input
                type="checkbox"
                checked={incidentTypes.includes(option.value)}
                onChange={() => toggleIncidentType(option.value)}
                className="h-4 w-4 rounded border-white/20 bg-white/[0.05] text-emerald-500 focus:ring-emerald-500/40 focus:ring-offset-0"
              />
              {option.label}
            </label>
          ))}
        </div>
        {touched && noIncidentTypes && (
          <p className="mt-1 text-sm text-red-400">Choose at least one alert type.</p>
        )}
      </fieldset>

      <div className="mt-5">
        <label htmlFor="minimum-severity" className="block text-sm font-medium text-slate-300">
          Minimum severity
        </label>
        <select
          id="minimum-severity"
          value={minimumSeverity}
          onChange={(event) =>
            setMinimumSeverity((event.target as HTMLSelectElement).value as Severity)
          }
          className="mt-2.5 w-full rounded-xl border border-white/[0.08] bg-white/[0.03] px-3 py-2.5 text-sm text-white transition-all duration-300 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
        >
          {SEVERITY_OPTIONS.map((option) => (
            <option key={option.value} value={option.value} className="bg-slate-900 text-white">
              {option.label}
            </option>
          ))}
        </select>
      </div>

      <div className="mt-5">
        <label htmlFor="geography" className="block text-sm font-medium text-slate-300">
          Area
        </label>
        <p className="mt-1 text-sm text-slate-500">
          A city, neighborhood, or region name -- e.g. "Douala" or "Bonamoussadi".
        </p>
        <input
          id="geography"
          type="text"
          list="cameroon-towns"
          value={geography}
          onInput={(event) => setGeography((event.target as HTMLInputElement).value)}
          onBlur={() => setTouched(true)}
          placeholder="Douala"
          className="mt-2.5 w-full rounded-xl border border-white/[0.08] bg-white/[0.03] px-3 py-2.5 text-sm text-white placeholder-slate-500 transition-all duration-300 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
        />
        <datalist id="cameroon-towns">
          {CAMEROON_TOWNS.map((town) => (
            <option key={town} value={town} />
          ))}
        </datalist>
        {touched && geographyIsBlank && (
          <p className="mt-1 text-sm text-red-400">Please enter an area.</p>
        )}
      </div>

      <button
        type="submit"
        disabled={submitting}
        className="group relative mt-6 w-full overflow-hidden rounded-xl bg-gradient-to-r from-emerald-600 to-emerald-700 px-4 py-3.5 text-base font-semibold text-white shadow-lg shadow-emerald-500/20 transition-all duration-300 hover:from-emerald-500 hover:to-emerald-600 hover:shadow-xl hover:shadow-emerald-500/30 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-60"
      >
        {submitting ? "Saving..." : submitLabel}
      </button>
    </form>
  );
}
