import type { ReactNode } from "react";

import { useAuth } from "../auth/AuthContext";

export function AppLayout({ children }: { children: ReactNode }) {
  const { session, logout } = useAuth();

  return (
    <div className="min-h-screen bg-slate-50">
      <header className="flex items-center justify-between border-b border-slate-200 bg-white px-6 py-4">
        <h1 className="text-lg font-semibold text-slate-900">SafeCameroon Console</h1>
        <div className="flex items-center gap-4 text-sm text-slate-600">
          <span>Reviewer {session?.reviewerId}</span>
          <button
            type="button"
            onClick={() => void logout()}
            className="rounded border border-slate-300 px-3 py-1.5 text-slate-700"
          >
            Sign out
          </button>
        </div>
      </header>
      <main className="mx-auto max-w-5xl p-6">{children}</main>
    </div>
  );
}
