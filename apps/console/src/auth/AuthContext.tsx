import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

import * as authApi from "../api/auth";
import type { Role } from "../api/organizations";

export interface Session {
  reviewerId: string;
  email: string;
  role: Role;
  token: string;
}

interface AuthContextValue {
  session: Session | null;
  loginWithGoogle: (idToken: string) => Promise<void>;
  logout: () => Promise<void>;
}

// Held in memory only, never localStorage/sessionStorage: refreshing the tab
// re-requires login. This trades convenience for the smallest possible XSS
// exposure window on a console that can view sensitive case/reporter data
// (docs/SECURITY_PRIVACY.md: least privilege, data minimization).
const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<Session | null>(null);

  const loginWithGoogle = useCallback(async (idToken: string) => {
    const response = await authApi.loginWithGoogle(idToken);
    setSession({
      reviewerId: response.reviewer_id,
      email: response.email,
      role: response.role,
      token: response.token,
    });
  }, []);

  const logout = useCallback(async () => {
    const token = session?.token;
    setSession(null);
    if (token) {
      // Best-effort: the local session is already cleared above regardless
      // of whether the server-side revocation call succeeds.
      await authApi.logout(token).catch(() => undefined);
    }
  }, [session]);

  const value = useMemo(
    () => ({ session, loginWithGoogle, logout }),
    [session, loginWithGoogle, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
