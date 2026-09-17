import { createContext, useContext, useMemo, type ReactNode } from "react";

import { detectLocale, type Locale } from "./locale";
import { translations, type Translations } from "./translations";

const LanguageContext = createContext<{ locale: Locale; t: Translations } | null>(null);

export function LanguageProvider({ children }: { children: ReactNode }) {
  const value = useMemo(() => {
    const locale = detectLocale();
    return { locale, t: translations[locale] };
  }, []);

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
