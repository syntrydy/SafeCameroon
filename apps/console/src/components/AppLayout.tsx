import type { ReactNode } from "react";

import type { Role } from "../api/organizations";
import { useAuth } from "../auth/AuthContext";
import { useTranslation } from "../i18n/LanguageContext";
import type { Translations } from "../i18n/translations";
import { Footer } from "./Footer";
import { Sidebar } from "./Sidebar";

function roleLabel(role: Role | undefined, t: Translations): string {
  switch (role) {
    case "PLATFORM_ADMIN":
      return t.layout.platformAdmin;
    case "ORG_ADMIN":
      return t.layout.orgAdmin;
    default:
      return t.layout.reviewer;
  }
}

export function AppLayout({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const { session, logout } = useAuth();

  return (
    <div className="relative flex min-h-screen overflow-hidden bg-slate-950">
      <div aria-hidden="true" className="pointer-events-none fixed inset-0">
        <div className="absolute top-0 left-0 h-full w-full bg-[radial-gradient(ellipse_at_top_left,rgba(16,185,129,0.06)_0%,transparent_50%)]" />
        <div className="absolute bottom-0 right-0 h-full w-full bg-[radial-gradient(ellipse_at_bottom_right,rgba(59,130,246,0.05)_0%,transparent_50%)]" />
      </div>

      <Sidebar />

      <div className="relative flex flex-1 flex-col overflow-hidden">
        <header className="flex items-center justify-end gap-4 border-b border-white/[0.08] bg-slate-900/60 px-6 py-4 text-sm text-slate-400 backdrop-blur-xl">
          <span>
            {roleLabel(session?.role, t)} {session?.email}
          </span>
          <button
            type="button"
            onClick={() => void logout()}
            className="rounded-lg border border-white/[0.08] px-3 py-1.5 text-slate-300 transition-colors hover:bg-white/[0.05] hover:text-white"
          >
            {t.layout.signOut}
          </button>
        </header>
        <main className="relative mx-auto w-full max-w-5xl flex-1 p-6">{children}</main>
        <Footer />
      </div>
    </div>
  );
}
