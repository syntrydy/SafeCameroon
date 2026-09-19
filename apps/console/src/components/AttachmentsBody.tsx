import type { AttachmentSummary } from "../api/attachments";
import { useTranslation } from "../i18n/LanguageContext";

interface AttachmentsBodyProps {
  loading: boolean;
  error: string | null;
  attachments: AttachmentSummary[] | null;
  downloadUrls: Record<string, string>;
  onGetDownloadUrl: (attachmentId: string) => void;
}

// The expanded content of an attachments list -- rendered once the caller's
// own toggle has fetched the list via useReportAttachments.
export function AttachmentsBody({
  loading,
  error,
  attachments,
  downloadUrls,
  onGetDownloadUrl,
}: AttachmentsBodyProps) {
  const { t } = useTranslation();

  return (
    <>
      {loading && <p className="text-xs text-slate-500">{t.linkedAttachments.loading}</p>}
      {error && (
        <p role="alert" className="text-xs text-red-300">
          {error}
        </p>
      )}
      {attachments !== null && attachments.length === 0 && (
        <p className="text-xs text-slate-500">{t.linkedAttachments.none}</p>
      )}
      {attachments && attachments.length > 0 && (
        <ul>
          {attachments.map((attachment) => (
            <li key={attachment.attachment_id} className="mb-1 text-xs text-slate-300">
              {attachment.content_type} ({t.linkedAttachments.sizeBytes(attachment.size_bytes)}){" "}
              {downloadUrls[attachment.attachment_id] ? (
                <a
                  href={downloadUrls[attachment.attachment_id]}
                  target="_blank"
                  rel="noreferrer"
                  className="text-white underline"
                >
                  {t.linkedAttachments.open}
                </a>
              ) : (
                <button
                  type="button"
                  onClick={() => onGetDownloadUrl(attachment.attachment_id)}
                  className="text-slate-400 underline"
                >
                  {t.linkedAttachments.getDownloadLink}
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
