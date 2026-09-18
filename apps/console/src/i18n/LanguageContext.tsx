import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { detectLocale, type Locale } from "./locale";
import { translations, type Translations } from "./translations";

const STORAGE_KEY = "console.locale";

interface LanguageContextValue {
  locale: Locale;
  t: Translations;
  setLocale: (locale: Locale) => void;
}

const LanguageContext = createContext<LanguageContextValue | null>(null);

// An explicit choice always wins over the browser's own language setting --
// a shared/borrowed device, or a browser locale that doesn't match what the
// reviewer actually reads, are both real cases here.
function readStoredLocale(): Locale | null {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    return stored === "en" || stored === "fr" ? stored : null;
  } catch {
    return null;
  }
}

export function LanguageProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => readStoredLocale() ?? detectLocale());

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // best-effort only -- the choice still applies for this session
    }
  }, []);

  // Keeps the document's lang accurate for screen readers once the actual
  // locale (detected or chosen) is known.
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  const value = useMemo(() => ({ locale, t: translations[locale], setLocale }), [locale, setLocale]);

  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

/** `t` here is the whole translation tree for the current locale (typed,
 * so a missing key is a compile error rather than a silent blank string) --
 * not a `t("some.key")` lookup function. */
export function useTranslation(): LanguageContextValue {
  const context = useContext(LanguageContext);
  if (!context) {
    throw new Error("useTranslation must be used within a LanguageProvider");
  }
  return context;
}
