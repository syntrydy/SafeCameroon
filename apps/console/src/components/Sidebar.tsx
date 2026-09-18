import { useCallback, useState } from "react";
import { NavLink } from "react-router-dom";

import { useAuth } from "../auth/AuthContext";
import { BRAND_NAME } from "../config/brand";
import { useTranslation } from "../i18n/LanguageContext";
import {
  AlertsIcon,
  ChevronLeftIcon,
  OrganizationsIcon,
  ReportsIcon,
  SubscriptionsIcon,
} from "./icons/NavIcons";
import { ShieldIcon } from "./icons/ShieldIcon";

const STORAGE_KEY = "console.sidebarCollapsed";

// Per-viewer convenience only (which state a reviewer left the sidebar in),
// never load-bearing: if storage throws (private window, blocked site data)
// the sidebar just falls back to its default expanded state.
function readStoredCollapsed(): boolean {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function linkClassName({ isActive }: { isActive: boolean }): string {
  return `flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors ${
    isActive ? "bg-white/[0.08] text-white" : "text-slate-400 hover:bg-white/[0.05] hover:text-white"
  }`;
}

export function Sidebar() {
  const { t } = useTranslation();
  const { session } = useAuth();
  const [collapsed, setCollapsed] = useState(readStoredCollapsed);

  const toggle = useCallback(() => {
    setCollapsed((current) => {
      const next = !current;
      try {
        window.localStorage.setItem(STORAGE_KEY, next ? "1" : "0");
      } catch {
        // best-effort only, see readStoredCollapsed
      }
      return next;
    });
  }, []);

  return (
    <aside
      className={`relative flex flex-shrink-0 flex-col border-r border-white/[0.08] bg-slate-900/60 backdrop-blur-xl transition-[width] duration-200 ${
        collapsed ? "w-[68px]" : "w-60"
      }`}
    >
      <div className="flex items-center gap-3 px-4 py-5">
        <div className="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-gradient-to-br from-emerald-400 to-emerald-600 shadow-lg shadow-emerald-500/25">
          <ShieldIcon className="h-5 w-5 text-white" />
        </div>
        {!collapsed && (
          <span className="truncate text-sm font-semibold text-white">
            {BRAND_NAME} {t.brand.consoleBadge}
          </span>
        )}
      </div>

      <nav className="flex flex-1 flex-col gap-1 px-3">
        <NavLink to="/" end className={linkClassName}>
          <ReportsIcon className="h-5 w-5 flex-shrink-0" />
          {!collapsed && <span className="truncate">{t.layout.reports}</span>}
        </NavLink>
        <NavLink to="/alerts" className={linkClassName}>
          <AlertsIcon className="h-5 w-5 flex-shrink-0" />
          {!collapsed && <span className="truncate">{t.layout.alerts}</span>}
        </NavLink>
        <NavLink to="/subscriptions" className={linkClassName}>
          <SubscriptionsIcon className="h-5 w-5 flex-shrink-0" />
          {!collapsed && <span className="truncate">{t.layout.subscriptions}</span>}
        </NavLink>
        {session?.role === "ORG_ADMIN" && (
          <NavLink to="/my-organization" className={linkClassName}>
            <OrganizationsIcon className="h-5 w-5 flex-shrink-0" />
            {!collapsed && <span className="truncate">{t.layout.myOrganization}</span>}
          </NavLink>
        )}
        {session?.role === "PLATFORM_ADMIN" && (
          <NavLink to="/organizations" className={linkClassName}>
            <OrganizationsIcon className="h-5 w-5 flex-shrink-0" />
            {!collapsed && <span className="truncate">{t.layout.organizations}</span>}
          </NavLink>
        )}
      </nav>

      <button
        type="button"
        onClick={toggle}
        aria-label={collapsed ? t.layout.expandSidebar : t.layout.collapseSidebar}
        className="mx-3 mb-4 flex items-center justify-center rounded-lg border border-white/[0.08] py-2 text-slate-400 transition-colors hover:bg-white/[0.05] hover:text-white"
      >
        <ChevronLeftIcon className={`h-4 w-4 transition-transform ${collapsed ? "rotate-180" : ""}`} />
      </button>
    </aside>
  );
}
