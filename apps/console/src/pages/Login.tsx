import { useCallback, useState } from "react";
import { Navigate, useLocation } from "react-router-dom";

import { ApiError } from "../api/client";
import { useAuth } from "../auth/AuthContext";
import { GoogleSignInButton } from "../auth/GoogleSignInButton";
import { Footer } from "../components/Footer";

interface LocationState {
  from?: { pathname: string };
}

function ShieldMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" className={className}>
      <path
        d="M12 2.5l7.5 3v5.2c0 4.86-3.2 9.24-7.5 10.8-4.3-1.56-7.5-5.94-7.5-10.8V5.5l7.5-3z"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
        fill="currentColor"
        fillOpacity="0.08"
      />
      <path
        d="M8.5 12.2l2.4 2.4 4.6-5.1"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

// Abstract "protection network" motif -- reports flowing in from a
// community, met by a verified, central response. Deliberately not
// photographic: this platform handles reports about at-risk people, so no
// stock imagery of children or victims belongs on the sign-in screen.
function ProtectionNetworkIllustration({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 200 140" fill="none" aria-hidden="true" className={className}>
      <line x1="100" y1="70" x2="28" y2="18" stroke="currentColor" strokeWidth="1" strokeOpacity="0.35" />
      <line x1="100" y1="70" x2="172" y2="18" stroke="currentColor" strokeWidth="1" strokeOpacity="0.35" />
      <line x1="100" y1="70" x2="18" y2="102" stroke="currentColor" strokeWidth="1" strokeOpacity="0.35" />
      <line x1="100" y1="70" x2="182" y2="102" stroke="currentColor" strokeWidth="1" strokeOpacity="0.35" />
      <line x1="100" y1="70" x2="100" y2="132" stroke="currentColor" strokeWidth="1" strokeOpacity="0.35" />

      <circle cx="28" cy="18" r="4" fill="currentColor" fillOpacity="0.55" />
      <circle cx="172" cy="18" r="4" fill="currentColor" fillOpacity="0.55" />
      <circle cx="18" cy="102" r="4" fill="currentColor" fillOpacity="0.55" />
      <circle cx="182" cy="102" r="4" fill="currentColor" fillOpacity="0.55" />
      <circle cx="100" cy="132" r="4" fill="currentColor" fillOpacity="0.55" />

      <g transform="translate(78, 38) scale(1.8)">
        <path
          d="M12 2.5l7.5 3v5.2c0 4.86-3.2 9.24-7.5 10.8-4.3-1.56-7.5-5.94-7.5-10.8V5.5l7.5-3z"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinejoin="round"
          fill="currentColor"
          fillOpacity="0.18"
        />
        <path
          d="M8.5 12.2l2.4 2.4 4.6-5.1"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </g>
    </svg>
  );
}

export function Login() {
  const { session, loginWithGoogle } = useAuth();
  const location = useLocation();
  const [error, setError] = useState<{ message: string; requestId: string | null } | null>(null);

  const handleCredential = useCallback(
    async (idToken: string) => {
      setError(null);
      try {
        await loginWithGoogle(idToken);
      } catch (cause) {
        if (cause instanceof ApiError) {
          setError({ message: cause.message, requestId: cause.requestId });
        } else {
          setError({ message: "An unexpected error occurred.", requestId: null });
        }
      }
    },
    [loginWithGoogle],
  );

  if (session) {
    const state = location.state as LocationState | null;
    return <Navigate to={state?.from?.pathname ?? "/"} replace />;
  }

  return (
    <div className="flex min-h-screen flex-col bg-slate-50">
      <div className="flex flex-1">
      <div className="relative hidden w-1/2 flex-col justify-between overflow-hidden bg-gradient-to-br from-slate-950 via-blue-950 to-emerald-900 p-12 text-white lg:flex">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute -top-24 -right-24 h-80 w-80 rounded-full bg-emerald-500/20 blur-3xl"
        />
        <div
          aria-hidden="true"
          className="pointer-events-none absolute -bottom-32 -left-16 h-96 w-96 rounded-full bg-blue-500/20 blur-3xl"
        />

        <div className="relative flex items-center gap-3">
          <ShieldMark className="h-8 w-8 text-emerald-400" />
          <span className="text-xl font-semibold tracking-tight">SafeCameroon</span>
        </div>

        <ProtectionNetworkIllustration className="relative h-36 w-full text-emerald-300/80" />

        <div className="relative max-w-md">
          <h2 className="text-3xl font-semibold leading-tight text-white">
            Coordinated response for every reported incident.
          </h2>
          <p className="mt-4 text-base leading-relaxed text-blue-100/80">
            Verified reviewers triage reports, confirm cases, and issue alerts through a
            single accountable console&mdash;built for privacy, speed, and oversight.
          </p>
        </div>

        <p className="relative text-xs tracking-wide text-blue-200/60">
          Privacy-first civic-protection platform
        </p>
      </div>

      <div className="flex w-full flex-1 items-center justify-center px-6 py-12 lg:w-1/2">
        <div className="w-full max-w-sm">
          <div className="mb-8 flex items-center gap-3 lg:hidden">
            <ShieldMark className="h-7 w-7 text-emerald-700" />
            <span className="text-lg font-semibold text-slate-900">SafeCameroon</span>
          </div>

          <div className="rounded-2xl border border-slate-200 bg-white p-8 shadow-xl shadow-slate-900/5">
            <p className="text-xs font-semibold uppercase tracking-wider text-emerald-700">
              Organization console
            </p>
            <h1 className="mt-2 text-2xl font-semibold text-slate-900">SafeCameroon Console</h1>
            <p className="mt-2 text-sm text-slate-500">
              Sign in with your registered Google account to continue.
            </p>

            <div className="mt-8 flex justify-center">
              <GoogleSignInButton onCredential={(idToken) => void handleCredential(idToken)} />
            </div>

            {error && (
              <div
                role="alert"
                className="mt-6 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
              >
                <p>{error.message}</p>
                {error.requestId && (
                  <p className="mt-1 text-xs text-red-500">reference: {error.requestId}</p>
                )}
              </div>
            )}
          </div>

          <p className="mt-6 text-center text-xs text-slate-400">
            Access is limited to reviewers added by an administrator.
          </p>
        </div>
      </div>
      </div>
      <Footer />
    </div>
  );
}
