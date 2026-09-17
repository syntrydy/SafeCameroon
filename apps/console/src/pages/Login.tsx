import { useCallback, useEffect, useState } from "react";
import { Navigate, useLocation } from "react-router-dom";

import { ApiError } from "../api/client";
import { useAuth } from "../auth/AuthContext";
import { GoogleSignInButton } from "../auth/GoogleSignInButton";
import { Footer } from "../components/Footer";
import { ShieldIcon } from "../components/icons/ShieldIcon";
import { NetworkIllustration } from "../components/NetworkIllustration";
import { useTranslation } from "../i18n/LanguageContext";

interface LocationState {
  from?: { pathname: string };
}

export function Login() {
  const { t } = useTranslation();
  const { session, loginWithGoogle } = useAuth();
  const location = useLocation();
  const [error, setError] = useState<{ message: string; requestId: string | null } | null>(null);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  const handleCredential = useCallback(
    async (idToken: string) => {
      setError(null);
      try {
        await loginWithGoogle(idToken);
      } catch (cause) {
        if (cause instanceof ApiError) {
          setError({ message: cause.message, requestId: cause.requestId });
        } else {
          setError({ message: t.common.unexpectedError, requestId: null });
        }
      }
    },
    [loginWithGoogle, t],
  );

  if (session) {
    const state = location.state as LocationState | null;
    return <Navigate to={state?.from?.pathname ?? "/"} replace />;
  }

  return (
    <div className="flex min-h-screen flex-col overflow-hidden bg-slate-950">
      <div
        aria-hidden="true"
        className="pointer-events-none fixed inset-0 bg-[radial-gradient(ellipse_at_top_left,rgba(16,185,129,0.08)_0%,transparent_50%)]"
      />
      <div
        aria-hidden="true"
        className="pointer-events-none fixed inset-0 bg-[radial-gradient(ellipse_at_bottom_right,rgba(59,130,246,0.06)_0%,transparent_50%)]"
      />

      <div className="relative flex flex-1">
        <div className="relative hidden w-[55%] flex-col justify-between overflow-hidden p-12 lg:flex xl:p-16">
          <div
            aria-hidden="true"
            className="absolute -top-32 -right-32 h-[500px] w-[500px] animate-pulse-slow rounded-full bg-emerald-500/10 blur-[100px]"
          />
          <div
            aria-hidden="true"
            className="absolute -bottom-40 -left-20 h-[600px] w-[600px] animate-pulse-slower rounded-full bg-blue-600/8 blur-[120px]"
          />
          <div
            aria-hidden="true"
            className="absolute top-1/2 left-1/3 h-[300px] w-[300px] animate-float rounded-full bg-emerald-400/5 blur-[80px]"
          />
          <div
            aria-hidden="true"
            className="absolute inset-0 opacity-[0.03]"
            style={{
              backgroundImage:
                "linear-gradient(rgba(255,255,255,0.1) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.1) 1px, transparent 1px)",
              backgroundSize: "60px 60px",
            }}
          />

          <div
            className={`relative z-10 flex items-center gap-3 transition-all duration-700 ${
              mounted ? "translate-y-0 opacity-100" : "-translate-y-4 opacity-0"
            }`}
          >
            <div className="relative">
              <div className="absolute inset-0 rounded-xl bg-emerald-400/20 blur-lg" />
              <div className="relative flex h-11 w-11 items-center justify-center rounded-xl bg-gradient-to-br from-emerald-400 to-emerald-600 shadow-lg shadow-emerald-500/25">
                <ShieldIcon className="h-6 w-6 text-white" />
              </div>
            </div>
            <div>
              <span className="text-xl font-bold tracking-tight text-white">{t.brand.name}</span>
              <span className="ml-2 rounded-full border border-emerald-500/20 bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium tracking-wider text-emerald-400 uppercase">
                {t.brand.consoleBadge}
              </span>
            </div>
          </div>

          <div
            className={`relative z-10 flex flex-1 items-center justify-center transition-all delay-300 duration-1000 ${
              mounted ? "scale-100 opacity-100" : "scale-95 opacity-0"
            }`}
          >
            <NetworkIllustration />
          </div>

          <div
            className={`relative z-10 max-w-lg transition-all delay-500 duration-700 ${
              mounted ? "translate-y-0 opacity-100" : "translate-y-4 opacity-0"
            }`}
          >
            <h2 className="text-3xl leading-tight font-semibold text-white xl:text-4xl">
              {t.login.heroHeadingPrefix}{" "}
              <span className="bg-gradient-to-r from-emerald-400 to-blue-400 bg-clip-text text-transparent">
                {t.login.heroHeadingHighlight}
              </span>
            </h2>
            <p className="mt-4 text-base leading-relaxed text-slate-400">{t.login.heroSubtitle}</p>
            <div className="mt-8 flex items-center gap-6">
              <div className="flex items-center gap-2">
                <div className="h-2 w-2 animate-pulse rounded-full bg-emerald-400" />
                <span className="text-xs text-slate-500">{t.login.fullAuditTrail}</span>
              </div>
              <div className="flex items-center gap-2">
                <div className="h-2 w-2 animate-pulse rounded-full bg-blue-400" />
                <span className="text-xs text-slate-500">{t.login.privacyFirst}</span>
              </div>
            </div>
          </div>
        </div>

        <div className="relative flex w-full flex-1 items-center justify-center px-6 py-12 lg:w-[45%]">
          <div
            aria-hidden="true"
            className="absolute top-1/4 bottom-1/4 left-0 hidden w-px bg-gradient-to-b from-transparent via-emerald-500/20 to-transparent lg:block"
          />

          <div
            className={`w-full max-w-[400px] transition-all delay-200 duration-700 ${
              mounted ? "translate-y-0 opacity-100" : "translate-y-6 opacity-0"
            }`}
          >
            <div className="mb-10 flex items-center gap-3 lg:hidden">
              <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-emerald-400 to-emerald-600 shadow-lg shadow-emerald-500/25">
                <ShieldIcon className="h-5 w-5 text-white" />
              </div>
              <span className="text-lg font-bold text-white">{t.brand.name}</span>
            </div>

            <div className="relative">
              <div
                aria-hidden="true"
                className="absolute -inset-1 rounded-3xl bg-gradient-to-r from-emerald-500/10 via-blue-500/10 to-emerald-500/10 opacity-50 blur-xl"
              />

              <div className="relative rounded-2xl border border-white/[0.08] bg-slate-900/80 p-8 shadow-2xl shadow-black/20 backdrop-blur-xl">
                <div className="mb-8">
                  <p className="text-xs font-semibold tracking-wider text-emerald-400 uppercase">
                    {t.login.organizationConsole}
                  </p>
                  <h1 className="mt-2 text-2xl font-bold text-white">{t.login.welcomeBack}</h1>
                  <p className="mt-2 text-sm leading-relaxed text-slate-400">
                    {t.login.signInSubtitle}
                  </p>
                </div>

                <div className="relative my-6">
                  <div className="absolute inset-0 flex items-center">
                    <div className="w-full border-t border-white/[0.06]" />
                  </div>
                </div>

                <div className="flex justify-center">
                  <GoogleSignInButton onCredential={(idToken) => void handleCredential(idToken)} />
                </div>

                <div className="mt-6 flex items-center justify-center gap-2 rounded-lg border border-emerald-500/10 bg-emerald-500/5 px-3 py-2.5">
                  <svg
                    className="h-3.5 w-3.5 flex-shrink-0 text-emerald-400"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={2}
                    aria-hidden="true"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"
                    />
                  </svg>
                  <p className="text-xs text-slate-400">{t.login.secureNote}</p>
                </div>

                {error && (
                  <div
                    role="alert"
                    className="mt-6 rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-300"
                  >
                    <p>{error.message}</p>
                    {error.requestId && (
                      <p className="mt-1 text-xs text-red-400/80">reference: {error.requestId}</p>
                    )}
                  </div>
                )}
              </div>
            </div>

            <div className="mt-8 space-y-3 text-center">
              <p className="text-xs text-slate-500">{t.login.accessLimited}</p>
              <Footer />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
