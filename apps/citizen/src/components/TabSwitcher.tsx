export type Tab = "report" | "alerts";

interface TabSwitcherProps {
  active: Tab;
  onChange: (tab: Tab) => void;
}

export function TabSwitcher({ active, onChange }: TabSwitcherProps) {
  return (
    <div className="mb-6 grid grid-cols-2 gap-1 rounded-xl bg-slate-100 p-1">
      {(
        [
          { tab: "report", label: "Report" },
          { tab: "alerts", label: "Get alerts" },
        ] as const
      ).map(({ tab, label }) => (
        <button
          key={tab}
          type="button"
          onClick={() => onChange(tab)}
          aria-pressed={active === tab}
          className={`rounded-lg py-2 text-sm font-medium transition ${
            active === tab
              ? "bg-white text-slate-900 shadow-sm"
              : "text-slate-500 hover:text-slate-700"
          }`}
        >
          {label}
        </button>
      ))}
    </div>
  );
}
