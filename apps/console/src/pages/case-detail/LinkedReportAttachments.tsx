import { useEffect, useState } from "react";

import { ApiError } from "../../api/client";
import type { ExtractedFields } from "../../api/extractions";
import { getReport, type ReportSummary } from "../../api/reports";
import { AttachmentsBody } from "../../components/AttachmentsBody";
import { useReportAttachments } from "../../components/useReportAttachments";
import { useTranslation } from "../../i18n/LanguageContext";
import { ExtractionPanel } from "../review-queue/ExtractionPanel";

interface LinkedReportAttachmentsProps {
  token: string;
  reportId: string;
  // Forwarded to ExtractionPanel so a reviewer can pull this report's AI
  // extraction straight into the case's create-alert form.
  onApplyToAlertForm?: (fields: ExtractedFields) => void;
  // Forwarded to ExtractionPanel's `autoApply` -- see its doc comment.
  autoApplyExtraction?: boolean;
}

export function LinkedReportAttachments({
  token,
  reportId,
  onApplyToAlertForm,
  autoApplyExtraction,
}: LinkedReportAttachmentsProps) {
  const { t } = useTranslation();
  const { expanded, toggleExpanded, loading, error, attachments, downloadUrls, getDownloadUrl } =
    useReportAttachments(token, reportId);

  const [report, setReport] = useState<ReportSummary | null>(null);
  const [reportError, setReportError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setReport(null);
    setReportError(null);
    getReport(token, reportId)
      .then((value) => {
        if (!cancelled) setReport(value);
      })
      .catch((cause) => {
        if (!cancelled) setReportError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
      });
    return () => {
      cancelled = true;
    };
  }, [token, reportId, t]);

  return (
    <li className="py-2">
      <div className="flex items-center gap-2">
        <span className="font-mono text-xs text-slate-500">{reportId.slice(0, 8)}</span>
        {report && (
          <span className="text-xs text-slate-500">
            {report.source_channel} &middot; {new Date(report.received_at).toLocaleString()}
          </span>
        )}
      </div>

      {reportError && (
        <p role="alert" className="mt-1 text-xs text-red-300">
          {reportError}
        </p>
      )}
      {report && <p className="mt-1 whitespace-pre-wrap text-sm text-slate-300">{report.raw_content}</p>}

      <ExtractionPanel
        token={token}
        reportId={reportId}
        onApply={onApplyToAlertForm}
        autoApply={autoApplyExtraction}
      />

      <button
        type="button"
        onClick={() => void toggleExpanded()}
        className="mt-1 text-xs font-medium text-slate-400 underline"
      >
        {expanded ? t.linkedAttachments.hideAttachments : t.linkedAttachments.showAttachments}
      </button>

      {expanded && (
        <div className="mt-2 pl-4">
          <AttachmentsBody
            loading={loading}
            error={error}
            attachments={attachments}
            downloadUrls={downloadUrls}
            onGetDownloadUrl={(attachmentId) => void getDownloadUrl(attachmentId)}
          />
        </div>
      )}
    </li>
  );
}
