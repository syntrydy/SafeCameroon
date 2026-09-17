import { BRAND_NAME } from "../config/brand";
import { CITIZEN_URL } from "../config/links";
import { useTranslation } from "../i18n/LanguageContext";

export function Footer() {
  const { t } = useTranslation();

  return (
    <footer className="border-t border-white/[0.06] px-6 py-4 text-center text-xs text-slate-500">
      <p>
        {BRAND_NAME} &middot; {t.footer.tagline}
      </p>
      <p className="mt-1">{t.footer.copyright(new Date().getFullYear())}</p>
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
