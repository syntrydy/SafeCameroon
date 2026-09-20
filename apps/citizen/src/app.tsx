import { useEffect, useState } from "preact/hooks";

import { AlertsPanel } from "./components/AlertsPanel";
import { ConfirmationCard } from "./components/ConfirmationCard";
import { Footer } from "./components/Footer";
import { ReceiptsList } from "./components/ReceiptsList";
import { ReportForm, type ReportFormPhoto } from "./components/ReportForm";
import { ShieldMark } from "./components/ShieldMark";
import { StatusBanner } from "./components/StatusBanner";
import { TabSwitcher, type Tab } from "./components/TabSwitcher";
import { BRAND_NAME } from "./config/brand";
import { countPendingReports, startBackgroundSync, submitOrQueue } from "./offline/queue";
import { listReceipts, type Receipt } from "./offline/receipts";
import { useOnlineStatus } from "./offline/useOnlineStatus";
import type { IncidentType } from "./api/reports";
import { LanguageProvider, useTranslation } from "./i18n/LanguageContext";

type View =
  | { kind: "form" }
  | { kind: "sent"; referenceCode: string }
  | { kind: "queued" };

export function App() {
  return (
    <LanguageProvider>
      <AppContent />
    </LanguageProvider>
  );
}

// Clicking a received push notification lands here (sw.ts's
// notificationclick handler deep-links to "/?tab=alerts") so the citizen
// sees their subscription, not the report form "/" alone defaults to.
function initialTab(): Tab {
  if (typeof window === "undefined") {
    return "report";
  }
  return new URLSearchParams(window.location.search).get("tab") === "alerts" ? "alerts" : "report";
}

function AppContent() {
  const { t } = useTranslation();
  const online = useOnlineStatus();
  const [tab, setTab] = useState<Tab>(initialTab);
  const [view, setView] = useState<View>({ kind: "form" });
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [pendingCount, setPendingCount] = useState(0);
  const [receipts, setReceipts] = useState<Receipt[]>(() => listReceipts());
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  useEffect(() => {
    document.title = `${BRAND_NAME} – Report a Missing Child`;
  }, []);

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

  async function handleSubmit(
    content: string,
    incidentType: IncidentType,
    photo?: ReportFormPhoto,
  ) {
    setSubmitting(true);
    setErrorMessage(null);
    try {
      const outcome = await submitOrQueue(
        content,
        incidentType,
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
    <div className="relative min-h-screen overflow-hidden bg-slate-950 px-4 py-6 sm:py-10">
      <div aria-hidden="true" className="pointer-events-none fixed inset-0">
        <div className="absolute top-0 left-0 h-full w-full bg-[radial-gradient(ellipse_at_top_left,rgba(16,185,129,0.06)_0%,transparent_50%)]" />
        <div className="absolute bottom-0 right-0 h-full w-full bg-[radial-gradient(ellipse_at_bottom_right,rgba(59,130,246,0.05)_0%,transparent_50%)]" />
        <div className="absolute top-1/2 left-1/2 h-[600px] w-[600px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-emerald-500/[0.03] blur-[100px]" />
      </div>
      <div
        aria-hidden="true"
        className="pointer-events-none fixed top-20 -right-20 h-[300px] w-[300px] animate-pulse-slow rounded-full bg-emerald-500/5 blur-[80px]"
      />
      <div
        aria-hidden="true"
        className="pointer-events-none fixed bottom-20 -left-20 h-[250px] w-[250px] animate-pulse-slower rounded-full bg-blue-600/5 blur-[80px]"
      />

      <div className="relative mx-auto w-full max-w-md">
        <div
          className={`mb-5 flex items-center justify-center gap-3 transition-all duration-700 sm:mb-8 ${
            mounted ? "translate-y-0 opacity-100" : "-translate-y-4 opacity-0"
          }`}
        >
          <div className="relative">
            <div className="absolute inset-0 rounded-xl bg-emerald-400/20 blur-lg" />
            <div className="relative flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-emerald-400 to-emerald-600 shadow-lg shadow-emerald-500/25">
              <ShieldMark className="h-5 w-5 text-white" />
            </div>
          </div>
          <div>
            <span className="text-lg font-bold text-white">{BRAND_NAME}</span>
            <span className="ml-2 rounded-full border border-emerald-500/20 bg-emerald-500/10 px-2 py-0.5 text-[10px] font-medium tracking-wider text-emerald-400 uppercase">
              {t.brand.citizenBadge}
            </span>
          </div>
        </div>

        <div
          className={`mb-4 transition-all delay-100 duration-700 sm:mb-6 ${
            mounted ? "translate-y-0 opacity-100" : "translate-y-4 opacity-0"
          }`}
        >
          <TabSwitcher active={tab} onChange={setTab} />
        </div>

        <div
          className={`relative transition-all delay-200 duration-700 ${
            mounted ? "translate-y-0 opacity-100" : "translate-y-6 opacity-0"
          }`}
        >
          <div
            aria-hidden="true"
            className="absolute -inset-1 rounded-3xl bg-gradient-to-r from-emerald-500/5 via-blue-500/5 to-emerald-500/5 opacity-60 blur-xl"
          />
          <div className="relative rounded-2xl border border-white/[0.08] bg-slate-900/80 p-4 shadow-2xl shadow-black/20 backdrop-blur-xl sm:p-8">
            {tab === "report" && (
              <>
                <StatusBanner online={online} pendingCount={pendingCount} />

                {view.kind === "form" && (
                  <>
                    <h1 className="text-xl font-bold text-white">{t.reportForm.heading}</h1>
                    <p className="mt-1.5 text-sm text-slate-400">{t.reportForm.subtitle}</p>
                    <div className="mt-4 sm:mt-6">
                      <ReportForm
                        submitting={submitting}
                        errorMessage={errorMessage}
                        onSubmit={(content, incidentType, photo) =>
                          void handleSubmit(content, incidentType, photo)
                        }
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
        </div>

        {tab === "report" && <ReceiptsList receipts={receipts} />}

        <div
          className={`mt-5 transition-all delay-300 duration-700 sm:mt-8 ${
            mounted ? "translate-y-0 opacity-100" : "translate-y-4 opacity-0"
          }`}
        >
          <Footer />
        </div>
      </div>
    </div>
  );
}
