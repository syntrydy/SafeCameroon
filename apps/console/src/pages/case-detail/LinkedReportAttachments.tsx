import { useState } from "react";

import { createDownloadUrl, listAttachmentsForReport, type AttachmentSummary } from "../../api/attachments";
import { ApiError } from "../../api/client";

interface LinkedReportAttachmentsProps {
  token: string;
  reportId: string;
}

// Attachments are the only content a case detail screen can show for a
// linked report: there is still no single-report GET (apps/api/src/reports.rs),
// so the report's raw text itself is not fetchable here, only its id and
// whatever files were attached to it.
export function LinkedReportAttachments({ token, reportId }: LinkedReportAttachmentsProps) {
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
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    } finally {
      setLoading(false);
    }
  }

  async function getDownloadUrl(attachmentId: string) {
    try {
      const { download_url } = await createDownloadUrl(token, attachmentId);
      setDownloadUrls((current) => ({ ...current, [attachmentId]: download_url }));
    } catch (cause) {
      setError(cause instanceof ApiError ? cause.message : "An unexpected error occurred.");
    }
  }

  return (
    <li className="py-2">
      <div className="flex items-center gap-2">
        <span className="font-mono text-xs text-slate-500">{reportId.slice(0, 8)}</span>
        <button type="button" onClick={() => void toggleExpanded()} className="text-xs font-medium text-slate-600 underline">
          {expanded ? "Hide attachments" : "Show attachments"}
        </button>
      </div>

      {expanded && (
        <div className="mt-2 pl-4">
          {loading && <p className="text-xs text-slate-500">Loading attachments...</p>}
          {error && (
            <p role="alert" className="text-xs text-red-700">
              {error}
            </p>
          )}
          {attachments !== null && attachments.length === 0 && (
            <p className="text-xs text-slate-500">No attachments.</p>
          )}
          {attachments && attachments.length > 0 && (
            <ul>
              {attachments.map((attachment) => (
                <li key={attachment.attachment_id} className="mb-1 text-xs text-slate-700">
                  {attachment.content_type} ({attachment.size_bytes} bytes){" "}
                  {downloadUrls[attachment.attachment_id] ? (
                    <a
                      href={downloadUrls[attachment.attachment_id]}
                      target="_blank"
                      rel="noreferrer"
                      className="text-slate-900 underline"
                    >
                      Open
                    </a>
                  ) : (
                    <button
                      type="button"
                      onClick={() => void getDownloadUrl(attachment.attachment_id)}
                      className="text-slate-600 underline"
                    >
                      Get download link
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </li>
  );
}
