import { useMemo, useRef, useState } from "preact/hooks";

import type { IncidentType, PhotoContentType } from "../api/reports";
import { MAX_REPORT_CONTENT_CHARS, validateReportContent } from "../domain/reportValidation";
import { useTranslation } from "../i18n/LanguageContext";
import { VoiceRecorder } from "./VoiceRecorder";

const ACCEPTED_PHOTO_TYPES: Record<string, PhotoContentType> = {
  "image/jpeg": "image/jpeg",
  "image/png": "image/png",
  "image/webp": "image/webp",
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
  const { t } = useTranslation();
  const [incidentType, setIncidentType] = useState<IncidentType>("MISSING_CHILD");
  const [content, setContent] = useState("");
  const [inputMode, setInputMode] = useState<"speak" | "type">("type");
  const [photo, setPhoto] = useState<ReportFormPhoto | null>(null);
  const [photoError, setPhotoError] = useState<string | null>(null);
  const [touched, setTouched] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const incidentTypeOptions: { value: IncidentType; label: string }[] = [
    { value: "MISSING_CHILD", label: t.reportForm.incidentTypeMissingChild },
    { value: "OTHER_PROTECTION_INCIDENT", label: t.reportForm.incidentTypeOther },
  ];
  const contentGuidance: Record<IncidentType, { helper: string; placeholder: string }> = {
    MISSING_CHILD: {
      helper: t.reportForm.missingChildHelper,
      placeholder: t.reportForm.missingChildPlaceholder,
    },
    OTHER_PROTECTION_INCIDENT: {
      helper: t.reportForm.otherHelper,
      placeholder: t.reportForm.otherPlaceholder,
    },
  };

  const validationError = useMemo(() => validateReportContent(content), [content]);
  const charCount = [...content.trim()].length;
  const guidance = contentGuidance[incidentType];

  function handlePhotoChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) {
      return;
    }
    const contentType = ACCEPTED_PHOTO_TYPES[file.type];
    if (!contentType) {
      setPhotoError(t.reportForm.photoInvalidType);
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
      <span className="block text-sm font-medium text-slate-300">
        {t.reportForm.incidentTypeQuestion}
      </span>
      <div className="mt-2.5 grid grid-cols-2 gap-1 rounded-xl border border-white/[0.06] bg-white/[0.03] p-1">
        {incidentTypeOptions.map((option) => (
          <button
            key={option.value}
            type="button"
            onClick={() => setIncidentType(option.value)}
            aria-pressed={incidentType === option.value}
            className={`relative rounded-lg py-2.5 text-sm font-medium transition-all duration-300 ${
              incidentType === option.value
                ? "border border-white/[0.08] bg-white/[0.08] text-white shadow-sm"
                : "text-slate-400 hover:text-slate-300"
            }`}
          >
            {incidentType === option.value && (
              <div className="absolute inset-0 rounded-lg bg-gradient-to-r from-emerald-500/10 to-emerald-500/5" />
            )}
            <span className="relative">{option.label}</span>
          </button>
        ))}
      </div>

      <div className="mt-6 grid grid-cols-2 gap-1 rounded-xl border border-white/[0.06] bg-white/[0.03] p-1">
        <button
          type="button"
          onClick={() => setInputMode("speak")}
          aria-pressed={inputMode === "speak"}
          className={`rounded-lg py-2.5 text-sm font-medium transition-all duration-300 ${
            inputMode === "speak"
              ? "border border-white/[0.08] bg-white/[0.08] text-white shadow-sm"
              : "text-slate-400 hover:text-slate-300"
          }`}
        >
          {t.reportForm.speakTab}
        </button>
        <button
          type="button"
          onClick={() => setInputMode("type")}
          aria-pressed={inputMode === "type"}
          className={`rounded-lg py-2.5 text-sm font-medium transition-all duration-300 ${
            inputMode === "type"
              ? "border border-white/[0.08] bg-white/[0.08] text-white shadow-sm"
              : "text-slate-400 hover:text-slate-300"
          }`}
        >
          {t.reportForm.typeTab}
        </button>
      </div>

      {inputMode === "speak" ? (
        <div className="mt-3">
          <VoiceRecorder
            onConfirm={(transcript) => {
              setContent(transcript);
              setInputMode("type");
            }}
          />
        </div>
      ) : (
        <>
          <label htmlFor="report-content" className="mt-3 block text-sm font-medium text-slate-300">
            {t.reportForm.whatIsHappening}
          </label>
          <p className="mt-1 text-sm text-slate-500">{guidance.helper}</p>
          <textarea
            id="report-content"
            value={content}
            onInput={(event) => setContent((event.target as HTMLTextAreaElement).value)}
            onBlur={() => setTouched(true)}
            rows={7}
            placeholder={guidance.placeholder}
            className="mt-3 w-full resize-none rounded-xl border border-white/[0.08] bg-white/[0.03] px-4 py-3 text-base text-white placeholder-slate-500 shadow-sm transition-all duration-300 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
          />
          <div className="mt-1.5 flex items-center justify-between text-xs text-slate-500">
            <span>
              {touched && validationError === "EMPTY" && (
                <span className="text-red-400">{t.reportForm.validationEmpty}</span>
              )}
              {touched && validationError === "TOO_LONG" && (
                <span className="text-red-400">{t.reportForm.validationTooLong}</span>
              )}
            </span>
            <span>{t.reportForm.charCount(charCount, MAX_REPORT_CONTENT_CHARS)}</span>
          </div>
        </>
      )}

      <div className="mt-6">
        <span className="block text-sm font-medium text-slate-300">{t.reportForm.photoLabel}</span>
        <p className="mt-1 text-sm text-slate-500">{t.reportForm.photoHelper}</p>

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
              className="rounded-lg border border-white/[0.08] px-3 py-1.5 text-sm text-slate-400 hover:bg-white/[0.05] hover:text-slate-300"
            >
              {t.reportForm.removePhoto}
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => fileInputRef.current?.click()}
            className="mt-3 flex items-center gap-2 rounded-xl border border-dashed border-white/[0.08] px-4 py-3 text-sm font-medium text-slate-400 transition-all duration-300 hover:border-emerald-500/30 hover:bg-emerald-500/5 hover:text-emerald-400"
          >
            {t.reportForm.addPhoto}
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
        {photoError && <p className="mt-2 text-sm text-red-400">{photoError}</p>}
      </div>

      {errorMessage && (
        <div
          role="alert"
          className="mt-6 rounded-lg border border-red-500/20 bg-red-500/10 px-4 py-3 text-sm text-red-300"
        >
          {errorMessage}
        </div>
      )}

      <button
        type="submit"
        disabled={submitting}
        className="group relative mt-6 w-full overflow-hidden rounded-xl bg-gradient-to-r from-emerald-600 to-emerald-700 px-4 py-3.5 text-base font-semibold text-white shadow-lg shadow-emerald-500/20 transition-all duration-300 hover:from-emerald-500 hover:to-emerald-600 hover:shadow-xl hover:shadow-emerald-500/30 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-60"
      >
        {submitting ? t.reportForm.sending : t.reportForm.sendReport}
      </button>

      <div className="mt-5 flex items-center justify-center gap-2 rounded-lg border border-emerald-500/10 bg-emerald-500/5 px-3 py-2.5">
        <p className="text-xs text-slate-400">{t.reportForm.anonymousNote}</p>
      </div>
    </form>
  );
}
