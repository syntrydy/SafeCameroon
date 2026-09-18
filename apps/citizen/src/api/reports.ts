import { apiRequest } from "./client";

// Matches crates/domain/src/case.rs `IncidentType` (SCREAMING_SNAKE_CASE on
// the wire). A citizen's own guess, never authoritative -- a reviewer still
// decides when creating a case (apps/api/src/reports.rs `CreateReportRequest`).
export type IncidentType = "MISSING_CHILD" | "OTHER_PROTECTION_INCIDENT";

export interface SubmitReportResult {
  report_id: string;
  reference_code: string;
  status: string;
}

export function submitReport(
  content: string,
  idempotencyKey: string,
  incidentType?: IncidentType,
): Promise<SubmitReportResult> {
  return apiRequest<SubmitReportResult>("/v1/reports", {
    method: "POST",
    body: JSON.stringify(
      incidentType ? { content, incident_type: incidentType } : { content },
    ),
    idempotencyKey,
  });
}

export interface TranscribeAudioResult {
  transcript: string;
}

/** Sends a recorded voice clip to be transcribed (issue #160) -- never
 * stores the audio itself; the caller shows the transcript back to the
 * reporter for confirmation before it becomes report text. */
export function transcribeAudio(audio: Blob): Promise<TranscribeAudioResult> {
  return apiRequest<TranscribeAudioResult>("/v1/reports/transcribe-audio", {
    method: "POST",
    headers: { "Content-Type": audio.type || "application/octet-stream" },
    body: audio,
  });
}

export type PhotoContentType = "image/jpeg" | "image/png" | "image/webp";

interface RequestAttachmentUploadResult {
  attachment_id: string;
  object_key: string;
  upload_url: string;
  upload_expires_in_seconds: number;
}

/** SHA-256 hex digest via the browser's native crypto -- no extra
 * dependency, and it runs entirely offline (it's a local computation, not a
 * network call). The backend stores this but does not re-verify it against
 * the uploaded bytes yet, so any non-blank value would be accepted; a real
 * digest is still worth sending as a genuine integrity signal. */
export async function sha256Hex(blob: Blob): Promise<string> {
  const buffer = await blob.arrayBuffer();
  const digest = await crypto.subtle.digest("SHA-256", buffer);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

/** Requests a presigned URL, then PUTs the raw bytes directly to storage --
 * the API never sees the file itself (crates/application/src/attachment_workflow.rs). */
export async function uploadPhoto(
  reportId: string,
  photo: Blob,
  contentType: PhotoContentType,
): Promise<void> {
  const checksum = await sha256Hex(photo);
  const prepared = await apiRequest<RequestAttachmentUploadResult>(
    `/v1/reports/${reportId}/attachments`,
    {
      method: "POST",
      body: JSON.stringify({
        content_type: contentType,
        size_bytes: photo.size,
        checksum,
      }),
    },
  );

  const uploadResponse = await fetch(prepared.upload_url, {
    method: "PUT",
    body: photo,
  });
  if (!uploadResponse.ok) {
    throw new Error(`Photo upload failed with status ${uploadResponse.status}`);
  }
}
