import { useTranslation } from "../i18n/LanguageContext";

export function Footer() {
  const { t } = useTranslation();

  return (
    <footer className="border-t border-white/[0.06] px-6 py-4 text-center text-xs text-slate-500">
      <p>
        {t.brand.name} &middot; {t.footer.tagline}
      </p>
      <p className="mt-1">{t.footer.copyright(new Date().getFullYear())}</p>
    </footer>
  );
}
