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
  if (props.kind === "sent") {
    return (
      <div className="text-center">
        <p className="text-sm font-semibold uppercase tracking-wider text-emerald-400">
          Report received
        </p>
        <p className="mt-3 text-sm text-slate-400">
          Save this reference code. You can use it to follow up.
        </p>
        <p className="mt-4 rounded-xl border border-emerald-500/20 bg-emerald-500/10 px-4 py-4 font-mono text-2xl font-semibold tracking-wide text-emerald-300 select-all">
          {props.referenceCode}
        </p>
        <button
          type="button"
          onClick={props.onReportAnother}
          className="mt-6 rounded-xl border border-white/[0.08] px-4 py-2.5 text-sm font-medium text-slate-300 hover:bg-white/[0.05]"
        >
          Report another
        </button>
      </div>
    );
  }

  return (
    <div className="text-center">
      <p className="text-sm font-semibold uppercase tracking-wider text-amber-400">
        Saved on this device
      </p>
      <p className="mt-3 text-sm text-slate-400">
        You're offline right now, so this report is saved and will send automatically as soon as
        you're connected. You don't need to do anything else -- keep this app open or come back
        to it later.
      </p>
      <button
        type="button"
        onClick={props.onReportAnother}
        className="mt-6 rounded-xl border border-white/[0.08] px-4 py-2.5 text-sm font-medium text-slate-300 hover:bg-white/[0.05]"
      >
        Report another
      </button>
    </div>
  );
}
