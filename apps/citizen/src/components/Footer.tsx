import { CONSOLE_URL } from "../config/links";
import { useTranslation } from "../i18n/LanguageContext";

export function Footer() {
  const { t, locale, setLocale } = useTranslation();

  return (
    <footer className="text-center">
      <div className="flex items-center justify-center gap-2">
        <div className="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" />
        <p className="text-xs text-slate-500">{t.footer.tagline}</p>
      </div>
      <p className="mt-2 text-xs text-slate-600">{t.footer.copyright(new Date().getFullYear())}</p>
      <div className="mt-2 flex items-center justify-center gap-1.5 text-xs">
        <button
          type="button"
          onClick={() => setLocale("en")}
          aria-label={t.footer.switchToEnglish}
          aria-current={locale === "en" || undefined}
          className={locale === "en" ? "font-semibold text-slate-300" : "text-slate-500 hover:text-slate-300"}
        >
          EN
        </button>
        <span aria-hidden="true" className="text-slate-600">
          |
        </span>
        <button
          type="button"
          onClick={() => setLocale("fr")}
          aria-label={t.footer.switchToFrench}
          aria-current={locale === "fr" || undefined}
          className={locale === "fr" ? "font-semibold text-slate-300" : "text-slate-500 hover:text-slate-300"}
        >
          FR
        </button>
      </div>
      {CONSOLE_URL && (
        <p className="mt-2 text-xs">
          <a href={CONSOLE_URL} className="text-emerald-500 hover:text-emerald-400 hover:underline">
            {t.footer.consoleLink}
          </a>
        </p>
      )}
    </footer>
  );
}
