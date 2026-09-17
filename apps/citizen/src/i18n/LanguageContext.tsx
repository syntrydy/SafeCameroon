import { createContext } from "preact";
import type { ComponentChildren } from "preact";
import { useContext, useEffect, useMemo } from "preact/hooks";

import { detectLocale, type Locale } from "./locale";
import { translations, type Translations } from "./translations";

const LanguageContext = createContext<{ locale: Locale; t: Translations } | null>(null);

export function LanguageProvider({ children }: { children: ComponentChildren }) {
  const value = useMemo(() => {
    const locale = detectLocale();
    return { locale, t: translations[locale] };
  }, []);

  // Keeps the static index.html's lang="en" accurate for screen readers and
  // search engines once the actual detected locale is known -- the HTML
  // shell has no server-side locale detection to set this upfront.
  useEffect(() => {
    document.documentElement.lang = value.locale;
  }, [value.locale]);

  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

/** `t` here is the whole translation tree for the detected locale (typed,
 * so a missing key is a compile error rather than a silent blank string) --
 * not a `t("some.key")` lookup function. */
export function useTranslation(): { locale: Locale; t: Translations } {
  const context = useContext(LanguageContext);
  if (!context) {
    throw new Error("useTranslation must be used within a LanguageProvider");
  }
  return context;
}
