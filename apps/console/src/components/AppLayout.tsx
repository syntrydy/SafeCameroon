import type { ReactNode } from "react";
import { Link } from "react-router-dom";

import { useAuth } from "../auth/AuthContext";
import { useTranslation } from "../i18n/LanguageContext";
import { Footer } from "./Footer";

export function AppLayout({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const { session, logout } = useAuth();

  return (
    <div className="relative min-h-screen overflow-hidden bg-slate-950">
      <div aria-hidden="true" className="pointer-events-none fixed inset-0">
        <div className="absolute top-0 left-0 h-full w-full bg-[radial-gradient(ellipse_at_top_left,rgba(16,185,129,0.06)_0%,transparent_50%)]" />
        <div className="absolute bottom-0 right-0 h-full w-full bg-[radial-gradient(ellipse_at_bottom_right,rgba(59,130,246,0.05)_0%,transparent_50%)]" />
      </div>

      <header className="relative flex items-center justify-between border-b border-white/[0.08] bg-slate-900/60 px-6 py-4 backdrop-blur-xl">
        <div className="flex items-center gap-6">
          <h1 className="text-lg font-semibold text-white">
            {t.brand.name} {t.brand.consoleBadge}
          </h1>
          <nav className="flex gap-4 text-sm text-slate-400">
            <Link to="/" className="hover:text-white">
              {t.layout.reports}
            </Link>
            <Link to="/alerts" className="hover:text-white">
              {t.layout.alerts}
            </Link>
            <Link to="/subscriptions" className="hover:text-white">
              {t.layout.subscriptions}
            </Link>
          </nav>
        </div>
        <div className="flex items-center gap-4 text-sm text-slate-400">
          <span>
            {t.layout.reviewer} {session?.reviewerId}
          </span>
          <button
            type="button"
            onClick={() => void logout()}
            className="rounded-lg border border-white/[0.08] px-3 py-1.5 text-slate-300 transition-colors hover:bg-white/[0.05] hover:text-white"
          >
            {t.layout.signOut}
          </button>
        </div>
      </header>
      <main className="relative mx-auto max-w-5xl p-6">{children}</main>
      <div className="relative">
        <Footer />
      </div>
    </div>
  );
}
