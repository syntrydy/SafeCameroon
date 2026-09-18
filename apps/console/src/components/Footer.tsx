import { BRAND_NAME } from "../config/brand";
import { CITIZEN_URL } from "../config/links";
import { useTranslation } from "../i18n/LanguageContext";

export function Footer() {
  const { t, locale, setLocale } = useTranslation();

  return (
    <footer className="border-t border-white/[0.06] px-6 py-4 text-center text-xs text-slate-500">
      <p>
        {BRAND_NAME} &middot; {t.footer.tagline}
      </p>
      <p className="mt-1">{t.footer.copyright(new Date().getFullYear())}</p>
      <div className="mt-1 flex items-center justify-center gap-1.5">
        <button
          type="button"
          onClick={() => setLocale("en")}
          aria-label={t.footer.switchToEnglish}
          aria-current={locale === "en" || undefined}
          className={locale === "en" ? "font-semibold text-slate-300" : "hover:text-slate-300"}
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
          className={locale === "fr" ? "font-semibold text-slate-300" : "hover:text-slate-300"}
        >
          FR
        </button>
      </div>
      {CITIZEN_URL && (
        <p className="mt-1">
          <a href={CITIZEN_URL} className="text-emerald-500 hover:text-emerald-400 hover:underline">
            {t.footer.citizenLink}
          </a>
        </p>
      )}
    </footer>
  );
}
