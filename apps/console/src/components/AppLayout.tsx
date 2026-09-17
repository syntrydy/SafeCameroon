import type { ReactNode } from "react";
import { Link } from "react-router-dom";

import { useAuth } from "../auth/AuthContext";
import { useTranslation } from "../i18n/LanguageContext";
import { Footer } from "./Footer";

export function AppLayout({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const { session, logout } = useAuth();

  return (
    <div className="min-h-screen bg-slate-50">
      <header className="flex items-center justify-between border-b border-slate-200 bg-white px-6 py-4">
        <div className="flex items-center gap-6">
          <h1 className="text-lg font-semibold text-slate-900">
            {t.brand.name} {t.brand.consoleBadge}
          </h1>
          <nav className="flex gap-4 text-sm text-slate-600">
            <Link to="/" className="hover:text-slate-900 hover:underline">
              {t.layout.reports}
            </Link>
            <Link to="/alerts" className="hover:text-slate-900 hover:underline">
              {t.layout.alerts}
            </Link>
            <Link to="/subscriptions" className="hover:text-slate-900 hover:underline">
              {t.layout.subscriptions}
            </Link>
          </nav>
        </div>
        <div className="flex items-center gap-4 text-sm text-slate-600">
          <span>
            {t.layout.reviewer} {session?.reviewerId}
          </span>
          <button
            type="button"
            onClick={() => void logout()}
            className="rounded border border-slate-300 px-3 py-1.5 text-slate-700"
          >
            {t.layout.signOut}
          </button>
        </div>
      </header>
      <main className="mx-auto max-w-5xl p-6">{children}</main>
      <Footer />
    </div>
  );
}
