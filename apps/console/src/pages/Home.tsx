import { useAuth } from "../auth/AuthContext";

// Placeholder landing page. The review queue (prompts/10_ORG_CONSOLE.md)
// replaces this in a follow-on PR.
export function Home() {
  const { session, logout } = useAuth();

  return (
    <div className="mx-auto max-w-2xl p-8">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold text-slate-900">SafeCameroon Console</h1>
        <button
          type="button"
          onClick={() => void logout()}
          className="rounded border border-slate-300 px-3 py-1.5 text-sm text-slate-700"
        >
          Sign out
        </button>
      </div>
      <p className="mt-4 text-sm text-slate-600">Signed in as reviewer {session?.reviewerId}.</p>
    </div>
  );
}
