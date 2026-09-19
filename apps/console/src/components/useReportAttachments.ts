import { useState } from "react";

import { createDownloadUrl, listAttachmentsForReport, type AttachmentSummary } from "../api/attachments";
import { ApiError } from "../api/client";
import { useTranslation } from "../i18n/LanguageContext";

// Shared by LinkedReportAttachments (case detail) and ReportRow (review
// queue) -- both need the same lazy-load-behind-a-toggle, fetch-download-
// link-on-demand behavior, just in different table/list layouts, so the
// async logic lives here and each caller renders its own markup.
export function useReportAttachments(token: string, reportId: string) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [attachments, setAttachments] = useState<AttachmentSummary[] | null>(null);
  const [downloadUrls, setDownloadUrls] = useState<Record<string, string>>({});

  async function toggleExpanded() {
    setExpanded((value) => !value);
    if (attachments !== null) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      setAttachments(await listAttachmentsForReport(token, reportId));
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    } finally {
      setLoading(false);
    }
  }

  async function getDownloadUrl(attachmentId: string) {
    try {
      const { download_url } = await createDownloadUrl(token, attachmentId);
      setDownloadUrls((current) => ({ ...current, [attachmentId]: download_url }));
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : t.common.unexpectedError);
    }
  }

  return { expanded, toggleExpanded, loading, error, attachments, downloadUrls, getDownloadUrl };
}
