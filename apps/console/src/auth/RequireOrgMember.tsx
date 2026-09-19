import type { ReactNode } from "react";
import { Navigate } from "react-router-dom";

import { useAuth } from "./AuthContext";

/** Renders `children` for any reviewer belonging to an organization --
 * `MEMBER`, `ORG_ADMIN`, or `PLATFORM_ADMIN` -- since this is the read side
 * of "my organization" (org detail, member list, own subscriptions) every
 * reviewer lands on after login, not just an org's admin. A `PLATFORM_ADMIN`
 * has no organization of their own (`Organization.tsx`/`Organizations.tsx`
 * is their equivalent page), so this guard only blocks a session with no
 * role at all. Must be nested inside `RequireAuth`, which guarantees
 * `session` is non-null here. */
export function RequireOrgMember({ children }: { children: ReactNode }) {
  const { session } = useAuth();

  if (!session?.role) {
    return <Navigate to="/" replace />;
  }

  return <>{children}</>;
}
