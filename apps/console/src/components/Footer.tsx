import { useTranslation } from "../i18n/LanguageContext";

interface FooterProps {
  variant?: "light" | "dark";
}

export function Footer({ variant = "light" }: FooterProps) {
  const { t } = useTranslation();

  if (variant === "dark") {
    return (
      <footer className="text-center text-xs text-slate-500">
        <p>
          {t.brand.name} &middot; {t.footer.tagline}
        </p>
        <p className="mt-1">{t.footer.copyright(new Date().getFullYear())}</p>
      </footer>
    );
  }

  return (
    <footer className="border-t border-slate-200 bg-white px-6 py-4 text-center text-xs text-slate-400">
      <p>
        {t.brand.name} &middot; {t.footer.tagline}
      </p>
      <p className="mt-1">{t.footer.copyright(new Date().getFullYear())}</p>
    </footer>
  );
}
