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
import { useAuth } from "../../auth/AuthContext";
import { canCreateAlert } from "../../domain/alertEligibility";
import { nextStatusOptions } from "../../domain/caseTransitions";
import { CreateAlertForm } from "../alerts/CreateAlertForm";
import { LinkedReportAttachments } from "./LinkedReportAttachments";

const STATUS_LABELS: Record<CaseStatus, string> = {
  REPORTED: "Reported",
  UNDER_REVIEW: "Under review",
  VERIFIED: "Verified",
  ACTIVE: "Active",
  RESOLVED: "Resolved",
  CANCELLED: "Cancelled",
  REJECTED: "Rejected",
};

async function applyTransition(token: string, caseId: string, to: CaseStatus): Promise<Case> {
  if (to === "VERIFIED") return verifyCase(token, caseId);
  if (to === "RESOLVED") return resolveCase(token, caseId);
  return createCaseEvent(token, caseId, to);
}

export function CaseDetail() {
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

  const load = useCallback(async () => {
    if (!id) return;
    setLoading(true);
    setError(null);
    try {
      const [nextCase, nextEvents] = await Promise.all([getCase(token, id), listCaseEvents(token, id)]);
      setCaseData(nextCase);
      setEvents(nextEvents);
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setLoading(false);
    }
  }, [token, id]);

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
      setActionMessage(`Case moved to ${STATUS_LABELS[to]}.`);
      await load();
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setPendingTransition(null);
    }
  }

  if (loading) {
    return <p className="text-sm text-slate-500">Loading case...</p>;
  }

  if (error && !caseData) {
    return (
      <p role="alert" className="rounded bg-red-50 px-3 py-2 text-sm text-red-700">
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
          <h2 className="text-base font-semibold text-slate-900">Case {caseData.case_id.slice(0, 8)}</h2>
          <p className="text-sm text-slate-500">
            {caseData.incident_type.replace(/_/g, " ")} &middot; version {caseData.version}
          </p>
        </div>
        <span className="rounded bg-slate-100 px-2 py-1 text-xs font-medium text-slate-700">
          {STATUS_LABELS[caseData.status]}
        </span>
      </div>

      {actionMessage && (
        <p role="status" className="mb-4 rounded bg-emerald-50 px-3 py-2 text-sm text-emerald-700">
          {actionMessage}
        </p>
      )}
      {error && (
        <p role="alert" className="mb-4 rounded bg-red-50 px-3 py-2 text-sm text-red-700">
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
            className="rounded bg-slate-900 px-3 py-1.5 text-sm font-medium text-white disabled:opacity-50"
          >
            Mark {STATUS_LABELS[status]}
          </button>
        ))}
      </div>

      <section className="mb-6">
        <h3 className="mb-2 text-sm font-semibold text-slate-900">Linked reports</h3>
        {caseData.report_ids.length === 0 ? (
          <p className="text-sm text-slate-500">No linked reports.</p>
        ) : (
          <ul className="divide-y divide-slate-100">
            {caseData.report_ids.map((reportId) => (
              <LinkedReportAttachments key={reportId} token={token} reportId={reportId} />
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
          />
        </section>
      )}

      <section>
        <h3 className="mb-2 text-sm font-semibold text-slate-900">History</h3>
        <ul className="divide-y divide-slate-100">
          {events.map((event) => (
            <li key={event.id} className="py-2 text-sm text-slate-700">
              <span className="font-medium">{event.event_type.replace(/_/g, " ")}</span>{" "}
              <span className="text-slate-500">{new Date(event.occurred_at).toLocaleString()}</span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
