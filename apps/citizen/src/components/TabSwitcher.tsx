export type Tab = "report" | "alerts";

interface TabSwitcherProps {
  active: Tab;
  onChange: (tab: Tab) => void;
}

export function TabSwitcher({ active, onChange }: TabSwitcherProps) {
  return (
    <div className="grid grid-cols-2 gap-1 rounded-xl border border-white/[0.06] bg-white/[0.03] p-1 backdrop-blur-sm">
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
          className={`relative rounded-lg py-2.5 text-sm font-medium transition-all duration-300 ${
            active === tab
              ? "border border-white/[0.08] bg-white/[0.08] text-white shadow-sm"
              : "text-slate-400 hover:text-slate-300"
          }`}
        >
          {active === tab && (
            <div className="absolute inset-0 rounded-lg bg-gradient-to-r from-emerald-500/10 to-blue-500/10" />
          )}
          <span className="relative">{label}</span>
        </button>
      ))}
    </div>
  );
}
