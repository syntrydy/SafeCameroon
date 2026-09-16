import type { Comparison, SubscriptionRule } from "../../api/subscriptions";

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
const COMPARISONS: { value: Comparison; label: string }[] = [
  { value: "GREATER_THAN", label: "> greater than" },
  { value: "GREATER_THAN_OR_EQUAL", label: ">= greater than or equal" },
  { value: "EQUAL", label: "== equal" },
  { value: "LESS_THAN_OR_EQUAL", label: "<= less than or equal" },
  { value: "LESS_THAN", label: "< less than" },
];

function defaultRuleFor(ruleType: SubscriptionRule["rule"]): SubscriptionRule {
  switch (ruleType) {
    case "INCIDENT_TYPE":
      return { rule: "INCIDENT_TYPE", values: [] };
    case "SEVERITY":
      return { rule: "SEVERITY", operator: "GREATER_THAN_OR_EQUAL", value: "HIGH" };
    case "EVENT_TYPE":
      return { rule: "EVENT_TYPE", values: [] };
    case "GEOGRAPHY":
      return { rule: "GEOGRAPHY", area: "" };
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
  function updateRule(index: number, rule: SubscriptionRule) {
    onChange(rules.map((existing, i) => (i === index ? rule : existing)));
  }

  function removeRule(index: number) {
    onChange(rules.filter((_, i) => i !== index));
  }

  return (
    <div>
      {rules.map((rule, index) => (
        <div key={index} className="mb-3 rounded border border-slate-200 p-3">
          <div className="mb-2 flex items-center justify-between">
            <select
              aria-label={`Rule ${index + 1} type`}
              value={rule.rule}
              onChange={(event) => updateRule(index, defaultRuleFor(event.target.value as SubscriptionRule["rule"]))}
              className="rounded border border-slate-300 px-2 py-1 text-sm"
            >
              <option value="INCIDENT_TYPE">Incident type</option>
              <option value="SEVERITY">Severity</option>
              <option value="EVENT_TYPE">Event type</option>
              <option value="GEOGRAPHY">Geography</option>
            </select>
            <button
              type="button"
              onClick={() => removeRule(index)}
              className="text-xs font-medium text-red-700 underline"
            >
              Remove
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
                className="rounded border border-slate-300 px-2 py-1"
              >
                {COMPARISONS.map((option) => (
                  <option key={option.value} value={option.value}>
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
                className="rounded border border-slate-300 px-2 py-1"
              >
                {SEVERITIES.map((value) => (
                  <option key={value} value={value}>
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
            <input
              aria-label={`Rule ${index + 1} area`}
              value={rule.area}
              onChange={(event) => updateRule(index, { ...rule, area: event.target.value })}
              placeholder="e.g. Douala"
              className="w-full max-w-xs rounded border border-slate-300 px-2 py-1 text-sm"
            />
          )}
        </div>
      ))}

      <button
        type="button"
        onClick={() => onChange([...rules, defaultRuleFor("INCIDENT_TYPE")])}
        className="text-sm font-medium text-slate-700 underline"
      >
        Add rule
      </button>
    </div>
  );
}
