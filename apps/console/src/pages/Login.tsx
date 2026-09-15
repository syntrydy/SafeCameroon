import { useCallback, useState } from "react";
import { Navigate, useLocation } from "react-router-dom";

import { ApiError } from "../api/client";
import { useAuth } from "../auth/AuthContext";
import { GoogleSignInButton } from "../auth/GoogleSignInButton";

interface LocationState {
  from?: { pathname: string };
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
    <div className="flex min-h-screen items-center justify-center bg-slate-50">
      <div className="w-full max-w-sm rounded-lg border border-slate-200 bg-white p-8 shadow-sm text-center">
        <h1 className="mb-6 text-lg font-semibold text-slate-900">SafeCameroon Console</h1>
        <p className="mb-6 text-sm text-slate-600">Sign in with your registered Google account.</p>

        <div className="flex justify-center">
          <GoogleSignInButton onCredential={(idToken) => void handleCredential(idToken)} />
        </div>

        {error && (
          <p role="alert" className="mt-4 text-sm text-red-700">
            {error.message}
            {error.requestId && (
              <span className="mt-1 block text-xs text-red-500">reference: {error.requestId}</span>
            )}
          </p>
        )}
      </div>
    </div>
  );
}
