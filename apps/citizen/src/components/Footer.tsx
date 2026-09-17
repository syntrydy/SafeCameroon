import { useTranslation } from "../i18n/LanguageContext";

export function Footer() {
  const { t } = useTranslation();

  return (
    <footer className="text-center">
      <div className="flex items-center justify-center gap-2">
        <div className="h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" />
        <p className="text-xs text-slate-500">{t.footer.tagline}</p>
      </div>
      <p className="mt-2 text-xs text-slate-600">{t.footer.copyright(new Date().getFullYear())}</p>
    </footer>
  );
}
