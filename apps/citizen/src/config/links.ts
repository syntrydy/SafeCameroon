/**
 * Per-deployment cross-links to the organization console. Optional and
 * unset by default -- a deployment without a console URL configured simply
 * shows no link, rather than a broken one.
 */
export const CONSOLE_URL: string | undefined = import.meta.env.VITE_CONSOLE_URL || undefined;
