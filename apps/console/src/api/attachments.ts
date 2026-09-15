import { apiRequest } from "./client";

// Matches apps/api/src/attachments.rs `AttachmentSummaryResponse`. Metadata
// only, never the file bytes or a live download URL — createDownloadUrl
// below is the only way to get one, and it's short-lived.
export interface AttachmentSummary {
  attachment_id: string;
  object_key: string;
  content_type: string;
  size_bytes: number;
  checksum: string;
}

interface DownloadUrlResponse {
  download_url: string;
  expires_in_seconds: number;
}

export function listAttachmentsForReport(token: string, reportId: string): Promise<AttachmentSummary[]> {
  return apiRequest<AttachmentSummary[]>(`/v1/reports/${reportId}/attachments`, { token });
}

export function createDownloadUrl(token: string, attachmentId: string): Promise<DownloadUrlResponse> {
  return apiRequest<DownloadUrlResponse>(`/v1/attachments/${attachmentId}/download-url`, { token });
}
