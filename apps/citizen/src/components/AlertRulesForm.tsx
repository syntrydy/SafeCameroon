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
        <legend className="text-sm font-medium text-slate-700">Alert type</legend>
        <div className="mt-2 space-y-2">
          {INCIDENT_TYPE_OPTIONS.map((option) => (
            <label key={option.value} className="flex items-center gap-2 text-sm text-slate-700">
              <input
                type="checkbox"
                checked={incidentTypes.includes(option.value)}
                onChange={() => toggleIncidentType(option.value)}
                className="h-4 w-4 rounded border-slate-300 text-emerald-700 focus:ring-emerald-600"
              />
              {option.label}
            </label>
          ))}
        </div>
        {touched && noIncidentTypes && (
          <p className="mt-1 text-sm text-red-600">Choose at least one alert type.</p>
        )}
      </fieldset>

      <div className="mt-5">
        <label htmlFor="minimum-severity" className="block text-sm font-medium text-slate-700">
          Minimum severity
        </label>
        <select
          id="minimum-severity"
          value={minimumSeverity}
          onChange={(event) =>
            setMinimumSeverity((event.target as HTMLSelectElement).value as Severity)
          }
          className="mt-2 w-full rounded-xl border border-slate-300 px-3 py-2.5 text-sm text-slate-900 focus:border-emerald-600 focus:outline-none focus:ring-2 focus:ring-emerald-600/30"
        >
          {SEVERITY_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </div>

      <div className="mt-5">
        <label htmlFor="geography" className="block text-sm font-medium text-slate-700">
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
          className="mt-2 w-full rounded-xl border border-slate-300 px-3 py-2.5 text-sm text-slate-900 focus:border-emerald-600 focus:outline-none focus:ring-2 focus:ring-emerald-600/30"
        />
        <datalist id="cameroon-towns">
          {CAMEROON_TOWNS.map((town) => (
            <option key={town} value={town} />
          ))}
        </datalist>
        {touched && geographyIsBlank && (
          <p className="mt-1 text-sm text-red-600">Please enter an area.</p>
        )}
      </div>

      <button
        type="submit"
        disabled={submitting}
        className="mt-6 w-full rounded-xl bg-emerald-700 px-4 py-3.5 text-base font-semibold text-white shadow-sm transition hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {submitting ? "Saving..." : submitLabel}
      </button>
    </form>
  );
}
