import type { ReactNode } from "react";
import { Navigate } from "react-router-dom";

import { useAuth } from "./AuthContext";

/** Renders `children` only for a `PLATFORM_ADMIN` session; every other
 * reviewer is redirected to their organization. Must be nested inside
 * `RequireAuth`, which guarantees `session` is non-null here. */
export function RequirePlatformAdmin({ children }: { children: ReactNode }) {
  const { session } = useAuth();

  if (session?.role !== "PLATFORM_ADMIN") {
    return <Navigate to="/my-organization" replace />;
  }

  return <>{children}</>;
}
