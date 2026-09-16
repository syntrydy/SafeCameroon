import { useMemo, useRef, useState } from "preact/hooks";

import type { IncidentType, PhotoContentType } from "../api/reports";
import { MAX_REPORT_CONTENT_CHARS, validateReportContent } from "../domain/reportValidation";

const ACCEPTED_PHOTO_TYPES: Record<string, PhotoContentType> = {
  "image/jpeg": "image/jpeg",
  "image/png": "image/png",
  "image/webp": "image/webp",
};

const INCIDENT_TYPE_OPTIONS: { value: IncidentType; label: string }[] = [
  { value: "MISSING_CHILD", label: "Missing child" },
  { value: "OTHER_PROTECTION_INCIDENT", label: "Other incident" },
];

const CONTENT_GUIDANCE: Record<IncidentType, { helper: string; placeholder: string }> = {
  MISSING_CHILD: {
    helper:
      "Describe the child and the situation: name, age, what they look like, where and when they were last seen. Every detail helps.",
    placeholder:
      "Example: My 8-year-old daughter Amina has not returned from school. She was last seen near Carrefour Bonamoussadi around 3pm today, wearing a blue school uniform...",
  },
  OTHER_PROTECTION_INCIDENT: {
    helper:
      "Describe what happened: who is involved, what you saw, and where and when it happened. Every detail helps.",
    placeholder:
      "Example: I saw a child being physically abused near the Bonamoussadi market around 5pm today...",
  },
};

export interface ReportFormPhoto {
  blob: Blob;
  contentType: PhotoContentType;
  previewUrl: string;
}

interface ReportFormProps {
  submitting: boolean;
  errorMessage: string | null;
  onSubmit: (content: string, incidentType: IncidentType, photo?: ReportFormPhoto) => void;
}

export function ReportForm({ submitting, errorMessage, onSubmit }: ReportFormProps) {
  const [incidentType, setIncidentType] = useState<IncidentType>("MISSING_CHILD");
  const [content, setContent] = useState("");
  const [photo, setPhoto] = useState<ReportFormPhoto | null>(null);
  const [photoError, setPhotoError] = useState<string | null>(null);
  const [touched, setTouched] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const validationError = useMemo(() => validateReportContent(content), [content]);
  const charCount = [...content.trim()].length;
  const guidance = CONTENT_GUIDANCE[incidentType];

  function handlePhotoChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) {
      return;
    }
    const contentType = ACCEPTED_PHOTO_TYPES[file.type];
    if (!contentType) {
      setPhotoError("Please choose a JPEG, PNG, or WebP photo.");
      return;
    }
    setPhotoError(null);
    if (photo) {
      URL.revokeObjectURL(photo.previewUrl);
    }
    setPhoto({ blob: file, contentType, previewUrl: URL.createObjectURL(file) });
  }

  function removePhoto() {
    if (photo) {
      URL.revokeObjectURL(photo.previewUrl);
    }
    setPhoto(null);
    if (fileInputRef.current) {
      fileInputRef.current.value = "";
    }
  }

  function handleSubmit(event: Event) {
    event.preventDefault();
    setTouched(true);
    if (validationError) {
      return;
    }
    onSubmit(content.trim(), incidentType, photo ?? undefined);
  }

  return (
    <form onSubmit={handleSubmit} noValidate>
      <span className="block text-sm font-medium text-slate-700">What kind of report is this?</span>
      <div className="mt-2 grid grid-cols-2 gap-1 rounded-xl bg-slate-100 p-1">
        {INCIDENT_TYPE_OPTIONS.map((option) => (
          <button
            key={option.value}
            type="button"
            onClick={() => setIncidentType(option.value)}
            aria-pressed={incidentType === option.value}
            className={`rounded-lg py-2 text-sm font-medium transition ${
              incidentType === option.value
                ? "bg-white text-slate-900 shadow-sm"
                : "text-slate-500 hover:text-slate-700"
            }`}
          >
            {option.label}
          </button>
        ))}
      </div>

      <label htmlFor="report-content" className="mt-6 block text-sm font-medium text-slate-700">
        What is happening?
      </label>
      <p className="mt-1 text-sm text-slate-500">{guidance.helper}</p>
      <textarea
        id="report-content"
        value={content}
        onInput={(event) => setContent((event.target as HTMLTextAreaElement).value)}
        onBlur={() => setTouched(true)}
        rows={7}
        autoFocus
        placeholder={guidance.placeholder}
        className="mt-3 w-full rounded-xl border border-slate-300 px-4 py-3 text-base text-slate-900 shadow-sm focus:border-emerald-600 focus:outline-none focus:ring-2 focus:ring-emerald-600/30"
      />
      <div className="mt-1 flex items-center justify-between text-xs text-slate-400">
        <span>
          {touched && validationError === "EMPTY" && (
            <span className="text-red-600">Please describe the situation.</span>
          )}
          {touched && validationError === "TOO_LONG" && (
            <span className="text-red-600">Please shorten this a little.</span>
          )}
        </span>
        <span>
          {charCount.toLocaleString()} / {MAX_REPORT_CONTENT_CHARS.toLocaleString()}
        </span>
      </div>

      <div className="mt-6">
        <span className="block text-sm font-medium text-slate-700">Photo (optional)</span>
        <p className="mt-1 text-sm text-slate-500">
          A recent photo helps a reviewer confirm the report. You can skip this if you don't have
          one.
        </p>

        {photo ? (
          <div className="mt-3 flex items-center gap-3">
            <img
              src={photo.previewUrl}
              alt="Selected"
              className="h-20 w-20 rounded-lg object-cover"
            />
            <button
              type="button"
              onClick={removePhoto}
              className="rounded-lg border border-slate-300 px-3 py-1.5 text-sm text-slate-600 hover:bg-slate-50"
            >
              Remove photo
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            className="mt-3 rounded-lg border border-dashed border-slate-300 px-4 py-2.5 text-sm font-medium text-slate-600 hover:border-emerald-600 hover:text-emerald-700"
          >
            Add a photo
          </button>
        )}
        <input
          ref={fileInputRef}
          type="file"
          accept="image/jpeg,image/png,image/webp"
          capture="environment"
          onChange={handlePhotoChange}
          className="hidden"
        />
        {photoError && <p className="mt-2 text-sm text-red-600">{photoError}</p>}
      </div>

      {errorMessage && (
        <div
          role="alert"
          className="mt-6 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700"
        >
          {errorMessage}
        </div>
      )}

      <button
        type="submit"
        disabled={submitting}
        className="mt-6 w-full rounded-xl bg-emerald-700 px-4 py-3.5 text-base font-semibold text-white shadow-sm transition hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
      >
        {submitting ? "Sending..." : "Send report"}
      </button>

      <p className="mt-4 text-center text-xs text-slate-400">
        This report is anonymous. No account or personal information is required.
      </p>
    </form>
  );
}
