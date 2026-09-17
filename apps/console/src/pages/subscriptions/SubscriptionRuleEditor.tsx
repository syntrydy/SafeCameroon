import { useState } from "react";

import type { Comparison, SubscriptionRule } from "../../api/subscriptions";
import { useTranslation } from "../../i18n/LanguageContext";
import type { Translations } from "../../i18n/translations";

const INCIDENT_TYPES = ["MISSING_CHILD", "OTHER_PROTECTION_INCIDENT"] as const;
const EVENT_TYPES = [
  "CASE_CREATED",
  "CASE_REPORT_LINKED",
  "CASE_UNDER_REVIEW",
  "CASE_VERIFIED",
  "CASE_ACTIVATED",
  "CASE_RESOLVED",
  "CASE_CANCELLED",
  "CASE_REJECTED",
] as const;
const SEVERITIES = ["LOW", "MEDIUM", "HIGH", "CRITICAL"] as const;

function comparisons(t: Translations): { value: Comparison; label: string }[] {
  return [
    { value: "GREATER_THAN", label: t.ruleEditor.comparisonGreaterThan },
    { value: "GREATER_THAN_OR_EQUAL", label: t.ruleEditor.comparisonGreaterThanOrEqual },
    { value: "EQUAL", label: t.ruleEditor.comparisonEqual },
    { value: "LESS_THAN_OR_EQUAL", label: t.ruleEditor.comparisonLessThanOrEqual },
    { value: "LESS_THAN", label: t.ruleEditor.comparisonLessThan },
  ];
}

function defaultRuleFor(ruleType: SubscriptionRule["rule"]): SubscriptionRule {
  switch (ruleType) {
    case "INCIDENT_TYPE":
      return { rule: "INCIDENT_TYPE", values: [] };
    case "SEVERITY":
      return { rule: "SEVERITY", operator: "GREATER_THAN_OR_EQUAL", value: "HIGH" };
    case "EVENT_TYPE":
      return { rule: "EVENT_TYPE", values: [] };
    case "GEOGRAPHY":
      return { rule: "GEOGRAPHY", areas: [] };
  }
}

function toggleValue<T>(values: T[], value: T): T[] {
  return values.includes(value) ? values.filter((v) => v !== value) : [...values, value];
}

interface SubscriptionRuleEditorProps {
  rules: SubscriptionRule[];
  onChange: (rules: SubscriptionRule[]) => void;
}

export function SubscriptionRuleEditor({ rules, onChange }: SubscriptionRuleEditorProps) {
  const { t } = useTranslation();
  const [geographyDrafts, setGeographyDrafts] = useState<Record<number, string>>({});

  function updateRule(index: number, rule: SubscriptionRule) {
    onChange(rules.map((existing, i) => (i === index ? rule : existing)));
  }

  function removeRule(index: number) {
    onChange(rules.filter((_, i) => i !== index));
  }

  function addGeographyArea(index: number) {
    const rule = rules[index];
    if (rule.rule !== "GEOGRAPHY") {
      return;
    }
    const draft = (geographyDrafts[index] ?? "").trim();
    if (!draft) {
      return;
    }
    updateRule(index, { ...rule, areas: [...rule.areas, draft] });
    setGeographyDrafts((current) => ({ ...current, [index]: "" }));
  }

  function removeGeographyArea(index: number, areaIndex: number) {
    const rule = rules[index];
    if (rule.rule !== "GEOGRAPHY") {
      return;
    }
    updateRule(index, { ...rule, areas: rule.areas.filter((_, i) => i !== areaIndex) });
  }

  return (
    <div>
      {rules.map((rule, index) => (
        <div key={index} className="mb-3 rounded border border-white/[0.08] p-3">
          <div className="mb-2 flex items-center justify-between">
            <select
              aria-label={`Rule ${index + 1} type`}
              value={rule.rule}
              onChange={(event) => updateRule(index, defaultRuleFor(event.target.value as SubscriptionRule["rule"]))}
              className="rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
            >
              <option value="INCIDENT_TYPE" className="bg-slate-900 text-white">{t.ruleEditor.ruleIncidentType}</option>
              <option value="SEVERITY" className="bg-slate-900 text-white">{t.ruleEditor.ruleSeverity}</option>
              <option value="EVENT_TYPE" className="bg-slate-900 text-white">{t.ruleEditor.ruleEventType}</option>
              <option value="GEOGRAPHY" className="bg-slate-900 text-white">{t.ruleEditor.ruleGeography}</option>
            </select>
            <button
              type="button"
              onClick={() => removeRule(index)}
              className="text-xs font-medium text-red-300 underline"
            >
              {t.ruleEditor.remove}
            </button>
          </div>

          {rule.rule === "INCIDENT_TYPE" && (
            <div className="flex flex-wrap gap-3 text-sm">
              {INCIDENT_TYPES.map((value) => (
                <label key={value} className="flex items-center gap-1">
                  <input
                    type="checkbox"
                    checked={rule.values.includes(value)}
                    onChange={() => updateRule(index, { ...rule, values: toggleValue(rule.values, value) })}
                  />
                  {value.replace(/_/g, " ")}
                </label>
              ))}
            </div>
          )}

          {rule.rule === "SEVERITY" && (
            <div className="flex flex-wrap gap-3 text-sm">
              <select
                aria-label={`Rule ${index + 1} operator`}
                value={rule.operator}
                onChange={(event) => updateRule(index, { ...rule, operator: event.target.value as Comparison })}
                className="rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-white focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
              >
                {comparisons(t).map((option) => (
                  <option key={option.value} value={option.value} className="bg-slate-900 text-white">
                    {option.label}
                  </option>
                ))}
              </select>
              <select
                aria-label={`Rule ${index + 1} severity`}
                value={rule.value}
                onChange={(event) =>
                  updateRule(index, { ...rule, value: event.target.value as (typeof SEVERITIES)[number] })
                }
                className="rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-white focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
              >
                {SEVERITIES.map((value) => (
                  <option key={value} value={value} className="bg-slate-900 text-white">
                    {value}
                  </option>
                ))}
              </select>
            </div>
          )}

          {rule.rule === "EVENT_TYPE" && (
            <div className="flex flex-wrap gap-3 text-sm">
              {EVENT_TYPES.map((value) => (
                <label key={value} className="flex items-center gap-1">
                  <input
                    type="checkbox"
                    checked={rule.values.includes(value)}
                    onChange={() => updateRule(index, { ...rule, values: toggleValue(rule.values, value) })}
                  />
                  {value.replace(/_/g, " ")}
                </label>
              ))}
            </div>
          )}

          {rule.rule === "GEOGRAPHY" && (
            <div>
              {rule.areas.length > 0 && (
                <div className="mb-2 flex flex-wrap gap-1.5">
                  {rule.areas.map((area, areaIndex) => (
                    <span
                      key={areaIndex}
                      className="inline-flex items-center gap-1 rounded bg-white/[0.08] px-2 py-1 text-xs text-slate-300"
                    >
                      {area}
                      <button
                        type="button"
                        onClick={() => removeGeographyArea(index, areaIndex)}
                        aria-label={t.ruleEditor.removeArea(area)}
                        className="text-slate-500 hover:text-red-300"
                      >
                        &times;
                      </button>
                    </span>
                  ))}
                </div>
              )}
              <div className="flex max-w-xs gap-2">
                <input
                  aria-label={`Rule ${index + 1} area`}
                  value={geographyDrafts[index] ?? ""}
                  onChange={(event) =>
                    setGeographyDrafts((current) => ({ ...current, [index]: event.target.value }))
                  }
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      addGeographyArea(index);
                    }
                  }}
                  placeholder={t.ruleEditor.geographyPlaceholder}
                  className="w-full rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-sm text-white placeholder-slate-500 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
                />
                <button
                  type="button"
                  onClick={() => addGeographyArea(index)}
                  className="rounded-lg border border-white/[0.08] px-2 py-1 text-xs text-slate-300 hover:bg-white/[0.05]"
                >
                  {t.ruleEditor.addArea}
                </button>
              </div>
            </div>
          )}
        </div>
      ))}

      <button
        type="button"
        onClick={() => onChange([...rules, defaultRuleFor("INCIDENT_TYPE")])}
        className="text-sm font-medium text-slate-300 underline"
      >
        {t.ruleEditor.addRule}
      </button>
    </div>
  );
}
