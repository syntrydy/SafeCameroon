import { useEffect, useState } from "preact/hooks";

import { AlertsPanel } from "./components/AlertsPanel";
import { ConfirmationCard } from "./components/ConfirmationCard";
import { Footer } from "./components/Footer";
import { ReceiptsList } from "./components/ReceiptsList";
import { ReportForm, type ReportFormPhoto } from "./components/ReportForm";
import { ShieldMark } from "./components/ShieldMark";
import { StatusBanner } from "./components/StatusBanner";
import { TabSwitcher, type Tab } from "./components/TabSwitcher";
import { countPendingReports, startBackgroundSync, submitOrQueue } from "./offline/queue";
import { listReceipts, type Receipt } from "./offline/receipts";
import { useOnlineStatus } from "./offline/useOnlineStatus";

type View =
  | { kind: "form" }
  | { kind: "sent"; referenceCode: string }
  | { kind: "queued" };

export function App() {
  const online = useOnlineStatus();
  const [tab, setTab] = useState<Tab>("report");
  const [view, setView] = useState<View>({ kind: "form" });
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pendingCount, setPendingCount] = useState(0);
  const [receipts, setReceipts] = useState<Receipt[]>(() => listReceipts());

  useEffect(() => {
    const refreshPendingCount = () => void countPendingReports().then(setPendingCount);
    refreshPendingCount();

    const stop = startBackgroundSync(() => {
      setReceipts(listReceipts());
      refreshPendingCount();
    });
    const interval = window.setInterval(refreshPendingCount, 5_000);

    return () => {
      stop();
      window.clearInterval(interval);
    };
  }, []);

  async function handleSubmit(content: string, photo?: ReportFormPhoto) {
    setSubmitting(true);
    setErrorMessage(null);
    try {
      const outcome = await submitOrQueue(
        content,
        photo ? { blob: photo.blob, contentType: photo.contentType } : undefined,
      );
      if (outcome.status === "sent") {
        setReceipts(listReceipts());
        setView({ kind: "sent", referenceCode: outcome.referenceCode });
      } else if (outcome.status === "queued") {
        setPendingCount((count) => count + 1);
        setView({ kind: "queued" });
      } else {
        setErrorMessage(outcome.message);
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="min-h-screen bg-slate-50 px-4 py-10">
      <div className="mx-auto w-full max-w-md">
        <div className="mb-8 flex items-center justify-center gap-2.5">
          <ShieldMark className="h-7 w-7 text-emerald-700" />
          <span className="text-lg font-semibold text-slate-900">SafeCameroon</span>
        </div>

        <TabSwitcher active={tab} onChange={setTab} />

        <div className="rounded-2xl border border-slate-200 bg-white p-6 shadow-xl shadow-slate-900/5 sm:p-8">
          {tab === "report" && (
            <>
              <StatusBanner online={online} pendingCount={pendingCount} />

              {view.kind === "form" && (
                <>
                  <h1 className="text-xl font-semibold text-slate-900">
                    Report a missing child
                  </h1>
                  <p className="mt-1 text-sm text-slate-500">
                    No account needed. Your identity is never recorded.
                  </p>
                  <div className="mt-6">
                    <ReportForm
                      submitting={submitting}
                      errorMessage={errorMessage}
                      onSubmit={(content, photo) => void handleSubmit(content, photo)}
                    />
                  </div>
                </>
              )}

              {view.kind === "sent" && (
                <ConfirmationCard
                  kind="sent"
                  referenceCode={view.referenceCode}
                  onReportAnother={() => setView({ kind: "form" })}
                />
              )}

              {view.kind === "queued" && (
                <ConfirmationCard
                  kind="queued"
                  onReportAnother={() => setView({ kind: "form" })}
                />
              )}
            </>
          )}

          {tab === "alerts" && <AlertsPanel />}
        </div>

        {tab === "report" && <ReceiptsList receipts={receipts} />}

        <Footer />
      </div>
    </div>
  );
}
