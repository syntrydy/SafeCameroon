export type Locale = "en" | "fr";

export const DEFAULT_LOCALE: Locale = "en";

/** Only French and English are supported; any other browser language (or no
 * browser language at all, e.g. in a test environment) falls back to
 * English. */
export function detectLocale(): Locale {
  if (typeof navigator === "undefined") {
    return DEFAULT_LOCALE;
  }
  const candidates = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const candidate of candidates) {
    if (!candidate) {
      continue;
    }
    const primary = candidate.toLowerCase().split("-")[0];
    if (primary === "fr") {
      return "fr";
    }
    if (primary === "en") {
      return "en";
    }
  }
  return DEFAULT_LOCALE;
}
