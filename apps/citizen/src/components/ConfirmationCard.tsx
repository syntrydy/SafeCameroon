import { useTranslation } from "../i18n/LanguageContext";

interface SentConfirmationProps {
  kind: "sent";
  referenceCode: string;
  onReportAnother: () => void;
}

interface QueuedConfirmationProps {
  kind: "queued";
  onReportAnother: () => void;
}

type ConfirmationCardProps = SentConfirmationProps | QueuedConfirmationProps;

export function ConfirmationCard(props: ConfirmationCardProps) {
  const { t } = useTranslation();

  if (props.kind === "sent") {
    return (
      <div className="text-center">
        <p className="text-sm font-semibold uppercase tracking-wider text-emerald-400">
          {t.confirmation.reportReceivedTitle}
        </p>
        <p className="mt-3 text-sm text-slate-400">{t.confirmation.reportReceivedBody}</p>
        <p className="mt-4 rounded-xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-4 font-mono text-2xl font-semibold tracking-wide text-emerald-300 select-all">
          {props.referenceCode}
        </p>
        <button
          type="button"
          onClick={props.onReportAnother}
          className="mt-6 rounded-xl border border-white/[0.08] px-4 py-2.5 text-sm font-medium text-slate-300 hover:bg-white/[0.05]"
        >
          {t.confirmation.reportAnother}
        </button>
      </div>
    );
  }

  return (
    <div className="text-center">
      <p className="text-sm font-semibold uppercase tracking-wider text-amber-400">
        {t.confirmation.savedOnDeviceTitle}
      </p>
      <p className="mt-3 text-sm text-slate-400">{t.confirmation.savedOnDeviceBody}</p>
      <button
        type="button"
        onClick={props.onReportAnother}
        className="mt-6 rounded-xl border border-white/[0.08] px-4 py-2.5 text-sm font-medium text-slate-300 hover:bg-white/[0.05]"
      >
        {t.confirmation.reportAnother}
      </button>
    </div>
  );
}
