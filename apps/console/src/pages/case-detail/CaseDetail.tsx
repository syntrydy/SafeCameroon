import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";

import {
  createCaseEvent,
  getCase,
  listCaseEvents,
  resolveCase,
  verifyCase,
  type Case,
  type CaseEvent,
  type CaseStatus,
} from "../../api/cases";
import { ApiError } from "../../api/client";
import type { ExtractedFields } from "../../api/extractions";
import { useAuth } from "../../auth/AuthContext";
import { canCreateAlert } from "../../domain/alertEligibility";
import { nextStatusOptions } from "../../domain/caseTransitions";
import { useTranslation } from "../../i18n/LanguageContext";
import type { Translations } from "../../i18n/translations";
import { CreateAlertForm } from "../alerts/CreateAlertForm";
import { LinkedReportAttachments } from "./LinkedReportAttachments";

function statusLabels(t: Translations): Record<CaseStatus, string> {
  return {
    REPORTED: t.caseDetail.statusReported,
    UNDER_REVIEW: t.caseDetail.statusUnderReview,
    VERIFIED: t.caseDetail.statusVerified,
    ACTIVE: t.caseDetail.statusActive,
    RESOLVED: t.caseDetail.statusResolved,
    CANCELLED: t.caseDetail.statusCancelled,
    REJECTED: t.caseDetail.statusRejected,
  };
}

async function applyTransition(token: string, caseId: string, to: CaseStatus): Promise<Case> {
  if (to === "VERIFIED") return verifyCase(token, caseId);
  if (to === "RESOLVED") return resolveCase(token, caseId);
  return createCaseEvent(token, caseId, to);
}

export function CaseDetail() {
  const { t } = useTranslation();
  const labels = statusLabels(t);
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { session } = useAuth();
  // Safe: this page only renders inside <RequireAuth>.
  const token = session!.token;

  const [caseData, setCaseData] = useState<Case | null>(null);
  const [events, setEvents] = useState<CaseEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [pendingTransition, setPendingTransition] = useState<CaseStatus | null>(null);
  const [suggestedFields, setSuggestedFields] = useState<{ fields: ExtractedFields; appliedAt: number } | null>(
    null,
  );

  const load = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    setError(null);
    try {
      const [nextCase, nextEvents] = await Promise.all([getCase(token, id), listCaseEvents(token, id)]);
      setCaseData(nextCase);
      setEvents(nextEvents);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setLoading(false);
    }
  }, [token, id, t]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleTransition(to: CaseStatus) {
    if (!id) return;
    setPendingTransition(to);
    setActionMessage(null);
    setError(null);
    try {
      await applyTransition(token, id, to);
      setActionMessage(t.caseDetail.caseMovedTo(labels[to]));
      await load();
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setPendingTransition(null);
    }
  }

  if (loading) {
    return <p className="text-sm text-slate-500">{t.caseDetail.loading}</p>;
  }

  if (error && !caseData) {
    return (
      <p role="alert" className="rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
        {error}
      </p>
    );
  }

  if (!caseData) {
    return null;
  }

  return (
    <div>
      <div className="mb-4 flex items-center justify-between">
        <div>
          <h1 className="text-base font-semibold text-white">
            {t.caseDetail.heading(caseData.case_id.slice(0, 8))}
          </h1>
          <p className="text-sm text-slate-500">
            {caseData.incident_type.replace(/_/g, " ")} &middot; {t.caseDetail.version(caseData.version)}
          </p>
        </div>
        <span className="rounded bg-white/[0.08] px-2 py-1 text-xs font-medium text-slate-300">
          {labels[caseData.status]}
        </span>
      </div>

      {actionMessage && (
        <p role="status" className="mb-4 rounded border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300">
          {actionMessage}
        </p>
      )}
      {error && (
        <p role="alert" className="mb-4 rounded border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-300">
          {error}
        </p>
      )}

      <div className="mb-6 flex flex-wrap gap-2">
        {nextStatusOptions(caseData.status).map((status) => (
          <button
            key={status}
            type="button"
            disabled={pendingTransition !== null}
            onClick={() => void handleTransition(status)}
            className="rounded-lg bg-gradient-to-r from-emerald-600 to-emerald-700 px-3 py-1.5 text-sm font-medium text-white transition-colors hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t.caseDetail.markStatus(labels[status])}
          </button>
        ))}
      </div>

      <section className="mb-6">
        <h2 className="mb-2 text-sm font-semibold text-white">{t.caseDetail.linkedReports}</h2>
        {caseData.report_ids.length === 0 ? (
          <p className="text-sm text-slate-500">{t.caseDetail.noLinkedReports}</p>
        ) : (
          <ul className="divide-y divide-white/[0.06]">
            {caseData.report_ids.map((reportId) => (
              <LinkedReportAttachments
                key={reportId}
                token={token}
                reportId={reportId}
                onApplyToAlertForm={(fields) => setSuggestedFields({ fields, appliedAt: Date.now() })}
              />
            ))}
          </ul>
        )}
      </section>

      {canCreateAlert(caseData.status, caseData.incident_type) && (
        <section className="mb-6">
          <CreateAlertForm
            token={token}
            caseId={caseData.case_id}
            onCreated={(alert) => navigate(`/alerts/${alert.alert_id}`)}
            suggestedFields={suggestedFields}
          />
        </section>
      )}

      <section>
        <h2 className="mb-2 text-sm font-semibold text-white">{t.caseDetail.history}</h2>
        <ul className="divide-y divide-white/[0.06]">
          {events.map((event) => (
            <li key={event.id} className="py-2 text-sm text-slate-300">
              <span className="font-medium">{event.event_type.replace(/_/g, " ")}</span>{" "}
              <span className="text-slate-500">{new Date(event.occurred_at).toLocaleString()}</span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
