import type { ReactNode } from "react";
import { Navigate } from "react-router-dom";

import { useAuth } from "./AuthContext";

/** Renders `children` only for an `ORG_ADMIN` or `PLATFORM_ADMIN` session;
 * a plain `MEMBER` is bounced back to the review queue. Must be nested
 * inside `RequireAuth`, which guarantees `session` is non-null here. */
export function RequireOrgAdmin({ children }: { children: ReactNode }) {
  const { session } = useAuth();

  if (session?.role !== "ORG_ADMIN" && session?.role !== "PLATFORM_ADMIN") {
    return <Navigate to="/" replace />;
  }

  return <>{children}</>;
}
