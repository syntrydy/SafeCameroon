import { useTranslation } from "../i18n/LanguageContext";

export function StatusBanner({ online, pendingCount }: { online: boolean; pendingCount: number }) {
  const { t } = useTranslation();

  if (!online) {
    return (
      <div className="mb-4 rounded-lg border border-amber-500/20 bg-amber-500/10 px-4 py-2.5 text-sm text-amber-300">
        {t.statusBanner.offline}
      </div>
    );
  }

  if (pendingCount > 0) {
    return (
      <div className="mb-4 rounded-lg border border-blue-500/20 bg-blue-500/10 px-4 py-2.5 text-sm text-blue-300">
        {t.statusBanner.sending(pendingCount)}
      </div>
    );
  }

  return null;
}
