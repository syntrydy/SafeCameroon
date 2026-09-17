import { useState } from "preact/hooks";

import type { AlertSubscriptionRules, IncidentType, Severity } from "../api/subscriptions";
import { CAMEROON_TOWNS } from "../domain/cameroonTowns";
import { useTranslation } from "../i18n/LanguageContext";

interface AlertRulesFormProps {
  initialRules?: AlertSubscriptionRules;
  submitting: boolean;
  submitLabel: string;
  onSubmit: (rules: AlertSubscriptionRules) => void;
}

const DEFAULT_RULES: AlertSubscriptionRules = {
  incidentTypes: ["MISSING_CHILD"],
  minimumSeverity: "HIGH",
  geography: [],
};

export function AlertRulesForm({
  initialRules,
  submitting,
  submitLabel,
  onSubmit,
}: AlertRulesFormProps) {
  const { t } = useTranslation();
  const [incidentTypes, setIncidentTypes] = useState<IncidentType[]>(
    initialRules?.incidentTypes ?? DEFAULT_RULES.incidentTypes,
  );
  const [minimumSeverity, setMinimumSeverity] = useState<Severity>(
    initialRules?.minimumSeverity ?? DEFAULT_RULES.minimumSeverity,
  );
  const [geography, setGeography] = useState<string[]>(
    initialRules?.geography ?? DEFAULT_RULES.geography,
  );
  const [touched, setTouched] = useState(false);

  const incidentTypeOptions: { value: IncidentType; label: string }[] = [
    { value: "MISSING_CHILD", label: t.alertRules.incidentTypeMissingChild },
    { value: "OTHER_PROTECTION_INCIDENT", label: t.alertRules.incidentTypeOtherProtection },
  ];
  const severityOptions: { value: Severity; label: string }[] = [
    { value: "LOW", label: t.alertRules.severityLow },
    { value: "MEDIUM", label: t.alertRules.severityMedium },
    { value: "HIGH", label: t.alertRules.severityHigh },
    { value: "CRITICAL", label: t.alertRules.severityCritical },
  ];

  // --- Alert type: a typeahead multiselect over a closed, 2-option set ---
  const [alertTypeQuery, setAlertTypeQuery] = useState("");
  const [alertTypeOpen, setAlertTypeOpen] = useState(false);
  const [alertTypeHighlighted, setAlertTypeHighlighted] = useState(-1);

  const availableIncidentTypeOptions = incidentTypeOptions.filter(
    (option) => !incidentTypes.includes(option.value),
  );
  const filteredIncidentTypeOptions =
    alertTypeQuery.trim().length > 0
      ? availableIncidentTypeOptions.filter((option) =>
          option.label.toLowerCase().includes(alertTypeQuery.trim().toLowerCase()),
        )
      : availableIncidentTypeOptions;

  function addIncidentType(value: IncidentType) {
    setIncidentTypes((current) => (current.includes(value) ? current : [...current, value]));
    setAlertTypeQuery("");
    setAlertTypeOpen(false);
    setAlertTypeHighlighted(-1);
  }

  function removeIncidentType(value: IncidentType) {
    setIncidentTypes((current) => current.filter((item) => item !== value));
  }

  function handleAlertTypeKeyDown(event: KeyboardEvent) {
    if (!alertTypeOpen || filteredIncidentTypeOptions.length === 0) {
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setAlertTypeHighlighted((index) => Math.min(index + 1, filteredIncidentTypeOptions.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setAlertTypeHighlighted((index) => Math.max(index - 1, 0));
    } else if (event.key === "Enter" && alertTypeHighlighted >= 0) {
      event.preventDefault();
      addIncidentType(filteredIncidentTypeOptions[alertTypeHighlighted].value);
    } else if (event.key === "Escape") {
      setAlertTypeOpen(false);
    }
  }

  // --- Area: a typeahead multiselect, free text or a suggested town ---
  const [geographyDraft, setGeographyDraft] = useState("");
  const [suggestionsOpen, setSuggestionsOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(-1);

  const filteredTowns =
    geographyDraft.trim().length > 0
      ? CAMEROON_TOWNS.filter(
          (town) =>
            !geography.includes(town) &&
            town.toLowerCase().includes(geographyDraft.trim().toLowerCase()),
        ).slice(0, 6)
      : [];

  function addGeographyArea(value: string) {
    const trimmed = value.trim();
    setGeographyDraft("");
    setSuggestionsOpen(false);
    setHighlightedIndex(-1);
    if (!trimmed || geography.includes(trimmed)) {
      return;
    }
    setGeography((current) => [...current, trimmed]);
  }

  function removeGeographyArea(value: string) {
    setGeography((current) => current.filter((area) => area !== value));
  }

  function handleGeographyKeyDown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      if (suggestionsOpen && highlightedIndex >= 0 && filteredTowns[highlightedIndex]) {
        addGeographyArea(filteredTowns[highlightedIndex]);
      } else if (geographyDraft.trim()) {
        addGeographyArea(geographyDraft);
      }
      return;
    }
    if (!suggestionsOpen || filteredTowns.length === 0) {
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setHighlightedIndex((index) => Math.min(index + 1, filteredTowns.length - 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setHighlightedIndex((index) => Math.max(index - 1, 0));
    } else if (event.key === "Escape") {
      setSuggestionsOpen(false);
    }
  }

  const geographyIsBlank = geography.length === 0;
  const noIncidentTypes = incidentTypes.length === 0;

  function handleSubmit(event: Event) {
    event.preventDefault();
    setTouched(true);
    if (geographyIsBlank || noIncidentTypes) {
      return;
    }
    onSubmit({ incidentTypes, minimumSeverity, geography });
  }

  return (
    <form onSubmit={handleSubmit} noValidate>
      <fieldset>
        <legend className="text-sm font-medium text-slate-300">{t.alertRules.alertType}</legend>
        <div className="mt-2.5 rounded-xl border border-white/[0.06] bg-white/[0.03] p-2">
          {incidentTypes.length > 0 && (
            <div className="mb-2 flex flex-wrap gap-1.5">
              {incidentTypes.map((value) => {
                const option = incidentTypeOptions.find((candidate) => candidate.value === value);
                if (!option) {
                  return null;
                }
                return (
                  <span
                    key={value}
                    className="inline-flex items-center gap-1.5 rounded-full border border-emerald-500/20 bg-emerald-500/10 px-2.5 py-1 text-xs text-emerald-300"
                  >
                    {option.label}
                    <button
                      type="button"
                      onClick={() => removeIncidentType(value)}
                      aria-label={t.alertRules.removeIncidentType(option.label)}
                      className="text-emerald-400/70 hover:text-emerald-200"
                    >
                      &times;
                    </button>
                  </span>
                );
              })}
            </div>
          )}
          {availableIncidentTypeOptions.length > 0 && (
            <div className="relative">
              <input
                type="text"
                autoComplete="off"
                role="combobox"
                aria-expanded={alertTypeOpen && filteredIncidentTypeOptions.length > 0}
                aria-autocomplete="list"
                aria-controls="alert-type-suggestions"
                aria-label={t.alertRules.alertType}
                value={alertTypeQuery}
                onInput={(event) => {
                  setAlertTypeQuery((event.target as HTMLInputElement).value);
                  setAlertTypeOpen(true);
                  setAlertTypeHighlighted(-1);
                }}
                onFocus={() => setAlertTypeOpen(true)}
                onBlur={() => setAlertTypeOpen(false)}
                onKeyDown={handleAlertTypeKeyDown}
                placeholder={t.alertRules.alertTypePlaceholder}
                className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-sm text-white placeholder-slate-500 transition-all duration-300 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
              />
              {alertTypeOpen && filteredIncidentTypeOptions.length > 0 && (
                <ul
                  id="alert-type-suggestions"
                  role="listbox"
                  className="absolute z-10 mt-1 w-full overflow-hidden rounded-xl border border-white/[0.08] bg-slate-900 py-1 shadow-xl shadow-black/40"
                >
                  {filteredIncidentTypeOptions.map((option, index) => (
                    <li key={option.value}>
                      <button
                        type="button"
                        role="option"
                        aria-selected={index === alertTypeHighlighted}
                        onMouseDown={(event) => event.preventDefault()}
                        onClick={() => addIncidentType(option.value)}
                        className={`block w-full px-3 py-2 text-left text-sm transition-colors ${
                          index === alertTypeHighlighted
                            ? "bg-emerald-500/10 text-emerald-300"
                            : "text-slate-300 hover:bg-white/[0.05]"
                        }`}
                      >
                        {option.label}
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>
        {touched && noIncidentTypes && (
          <p className="mt-1 text-sm text-red-400">{t.alertRules.chooseAtLeastOne}</p>
        )}
      </fieldset>

      <div className="mt-5">
        <label htmlFor="minimum-severity" className="block text-sm font-medium text-slate-300">
          {t.alertRules.minimumSeverity}
        </label>
        <select
          id="minimum-severity"
          value={minimumSeverity}
          onChange={(event) =>
            setMinimumSeverity((event.target as HTMLSelectElement).value as Severity)
          }
          className="mt-2.5 w-full rounded-xl border border-white/[0.08] bg-white/[0.03] px-3 py-2.5 text-sm text-white transition-all duration-300 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
        >
          {severityOptions.map((option) => (
            <option key={option.value} value={option.value} className="bg-slate-900 text-white">
              {option.label}
            </option>
          ))}
        </select>
      </div>

      <div className="mt-5">
        <label htmlFor="geography" className="block text-sm font-medium text-slate-300">
          {t.alertRules.area}
        </label>
        <p className="mt-1 text-sm text-slate-500">{t.alertRules.areaHelper}</p>
        {geography.length > 0 && (
          <div className="mt-2.5 flex flex-wrap gap-1.5">
            {geography.map((area) => (
              <span
                key={area}
                className="inline-flex items-center gap-1.5 rounded-full border border-emerald-500/20 bg-emerald-500/10 px-2.5 py-1 text-xs text-emerald-300"
              >
                {area}
                <button
                  type="button"
                  onClick={() => removeGeographyArea(area)}
                  aria-label={t.alertRules.removeArea(area)}
                  className="text-emerald-400/70 hover:text-emerald-200"
                >
                  &times;
                </button>
              </span>
            ))}
          </div>
        )}
        <div className="relative mt-2.5">
          <input
            id="geography"
            type="text"
            autoComplete="off"
            role="combobox"
            aria-expanded={suggestionsOpen && filteredTowns.length > 0}
            aria-autocomplete="list"
            aria-controls="geography-suggestions"
            value={geographyDraft}
            onInput={(event) => {
              setGeographyDraft((event.target as HTMLInputElement).value);
              setSuggestionsOpen(true);
              setHighlightedIndex(-1);
            }}
            onFocus={() => setSuggestionsOpen(true)}
            onBlur={() => {
              setTouched(true);
              setSuggestionsOpen(false);
              // Committing an unfinished draft on blur means tabbing or
              // clicking away never silently discards what was typed --
              // only an explicit chip removal does.
              if (geographyDraft.trim()) {
                addGeographyArea(geographyDraft);
              }
            }}
            onKeyDown={handleGeographyKeyDown}
            placeholder={t.alertRules.areaPlaceholder}
            className="w-full rounded-xl border border-white/[0.08] bg-white/[0.03] px-3 py-2.5 text-sm text-white placeholder-slate-500 transition-all duration-300 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
          {suggestionsOpen && filteredTowns.length > 0 && (
            <ul
              id="geography-suggestions"
              role="listbox"
              className="absolute z-10 mt-1 max-h-56 w-full overflow-y-auto rounded-xl border border-white/[0.08] bg-slate-900 py-1 shadow-xl shadow-black/40"
            >
              {filteredTowns.map((town, index) => (
                <li key={town}>
                  <button
                    type="button"
                    role="option"
                    aria-selected={index === highlightedIndex}
                    onMouseDown={(event) => event.preventDefault()}
                    onClick={() => addGeographyArea(town)}
                    className={`block w-full px-3 py-2 text-left text-sm transition-colors ${
                      index === highlightedIndex
                        ? "bg-emerald-500/10 text-emerald-300"
                        : "text-slate-300 hover:bg-white/[0.05]"
                    }`}
                  >
                    {town}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        {touched && geographyIsBlank && (
          <p className="mt-1 text-sm text-red-400">{t.alertRules.areaRequired}</p>
        )}
      </div>

      <button
        type="submit"
        disabled={submitting}
        className="group relative mt-6 w-full overflow-hidden rounded-xl bg-gradient-to-r from-emerald-600 to-emerald-700 px-4 py-3.5 text-base font-semibold text-white shadow-lg shadow-emerald-500/20 transition-all duration-300 hover:from-emerald-500 hover:to-emerald-600 hover:shadow-xl hover:shadow-emerald-500/30 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-60"
      >
        {submitting ? t.alerts.saving : submitLabel}
      </button>
    </form>
  );
}
