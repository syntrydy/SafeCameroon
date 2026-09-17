/**
 * Per-deployment cross-link to the public citizen app. Optional and unset
 * by default -- a deployment without a citizen URL configured simply shows
 * no link, rather than a broken one.
 */
export const CITIZEN_URL: string | undefined = import.meta.env.VITE_CITIZEN_URL || undefined;
