import { AttachmentsBody } from "../../components/AttachmentsBody";
import { useReportAttachments } from "../../components/useReportAttachments";
import { useTranslation } from "../../i18n/LanguageContext";

interface LinkedReportAttachmentsProps {
  token: string;
  reportId: string;
}

// Attachments are the only content a case detail screen can show for a
// linked report: there is still no single-report GET (apps/api/src/reports.rs),
// so the report's raw text itself is not fetchable here, only its id and
// whatever files were attached to it.
export function LinkedReportAttachments({ token, reportId }: LinkedReportAttachmentsProps) {
  const { t } = useTranslation();
  const { expanded, toggleExpanded, loading, error, attachments, downloadUrls, getDownloadUrl } =
    useReportAttachments(token, reportId);

  return (
    <li className="py-2">
      <div className="flex items-center gap-2">
        <span className="font-mono text-xs text-slate-500">{reportId.slice(0, 8)}</span>
        <button type="button" onClick={() => void toggleExpanded()} className="text-xs font-medium text-slate-400 underline">
          {expanded ? t.linkedAttachments.hideAttachments : t.linkedAttachments.showAttachments}
        </button>
      </div>

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
