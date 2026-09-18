import { useEffect, useRef, useState } from "preact/hooks";

import { transcribeAudio } from "../api/reports";
import { useTranslation } from "../i18n/LanguageContext";

const MAX_RECORDING_SECONDS = 120;

// Chrome/Edge only ever record webm; Firefox and Safari can do better --
// tried in this order so the *best* format each browser supports wins
// (issue #160: the backend forwards whatever this produces to the
// transcription provider as-is, so this list is a real integration risk,
// not just a preference).
const CANDIDATE_MIME_TYPES = ["audio/ogg;codecs=opus", "audio/webm;codecs=opus", "audio/mp4"];

function pickSupportedMimeType(): string | undefined {
  if (typeof MediaRecorder === "undefined") {
    return undefined;
  }
  return CANDIDATE_MIME_TYPES.find((type) => MediaRecorder.isTypeSupported(type));
}

function formatMmSs(totalSeconds: number): string {
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

type RecorderState =
  | { kind: "idle" }
  | { kind: "recording"; secondsElapsed: number }
  | { kind: "transcribing" }
  | { kind: "review" }
  | { kind: "error"; message: string };

interface VoiceRecorderProps {
  onConfirm: (transcript: string) => void;
}

/** Records up to 2 minutes, transcribes it, and lets the reporter review
 * and edit the transcript before it becomes report text (issue #160) --
 * the raw audio is never sent anywhere except the one transcription
 * request, and never stored: the parent only ever receives the confirmed
 * text via `onConfirm`, indistinguishable from typed text. */
export function VoiceRecorder({ onConfirm }: VoiceRecorderProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<RecorderState>({ kind: "idle" });
  const [transcriptDraft, setTranscriptDraft] = useState("");
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const streamRef = useRef<MediaStream | null>(null);
  const timerRef = useRef<number | null>(null);

  function stopStream() {
    streamRef.current?.getTracks().forEach((track) => track.stop());
    streamRef.current = null;
  }

  function clearTimer() {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }

  useEffect(() => {
    return () => {
      clearTimer();
      stopStream();
    };
  }, []);

  async function transcribe(blob: Blob) {
    setState({ kind: "transcribing" });
    try {
      const result = await transcribeAudio(blob);
      setTranscriptDraft(result.transcript);
      setState({ kind: "review" });
    } catch {
      setState({ kind: "error", message: t.voiceRecorder.transcriptionFailed });
    }
  }

  async function startRecording() {
    if (
      typeof navigator === "undefined" ||
      !navigator.mediaDevices?.getUserMedia ||
      typeof MediaRecorder === "undefined"
    ) {
      setState({ kind: "error", message: t.voiceRecorder.unsupported });
      return;
    }

    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch {
      setState({ kind: "error", message: t.voiceRecorder.micPermissionDenied });
      return;
    }

    streamRef.current = stream;
    const mimeType = pickSupportedMimeType();
    const recorder = mimeType ? new MediaRecorder(stream, { mimeType }) : new MediaRecorder(stream);
    chunksRef.current = [];

    recorder.ondataavailable = (event) => {
      if (event.data.size > 0) {
        chunksRef.current.push(event.data);
      }
    };
    recorder.onstop = () => {
      stopStream();
      clearTimer();
      const blob = new Blob(chunksRef.current, { type: recorder.mimeType || "audio/webm" });
      void transcribe(blob);
    };

    mediaRecorderRef.current = recorder;
    recorder.start();
    setState({ kind: "recording", secondsElapsed: 0 });

    timerRef.current = window.setInterval(() => {
      setState((current) => {
        if (current.kind !== "recording") {
          return current;
        }
        const next = current.secondsElapsed + 1;
        if (next >= MAX_RECORDING_SECONDS) {
          mediaRecorderRef.current?.stop();
        }
        return { kind: "recording", secondsElapsed: next };
      });
    }, 1000);
  }

  function stopRecording() {
    mediaRecorderRef.current?.stop();
  }

  function reRecord() {
    setTranscriptDraft("");
    setState({ kind: "idle" });
  }

  if (state.kind === "idle" || state.kind === "error") {
    return (
      <div>
        <p className="text-sm font-medium text-slate-300">{t.voiceRecorder.prompt}</p>
        <p className="mt-1 text-sm text-slate-500">{t.voiceRecorder.recordingHint}</p>
        {state.kind === "error" && (
          <p role="alert" className="mt-3 text-sm text-red-400">
            {state.message}
          </p>
        )}
        <button
          type="button"
          onClick={() => void startRecording()}
          className="mt-4 flex w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-emerald-600 to-emerald-700 px-4 py-3.5 text-base font-semibold text-white shadow-lg shadow-emerald-500/20 transition-all duration-300 hover:from-emerald-500 hover:to-emerald-600"
        >
          {t.voiceRecorder.startRecording}
        </button>
      </div>
    );
  }

  if (state.kind === "recording") {
    return (
      <div className="text-center">
        <div className="flex items-center justify-center gap-2">
          <div className="h-2.5 w-2.5 animate-pulse rounded-full bg-red-500" />
          <span className="font-mono text-lg text-white">
            {t.voiceRecorder.timeRemaining(formatMmSs(state.secondsElapsed))}
          </span>
        </div>
        <p className="mt-2 text-sm text-slate-400">{t.voiceRecorder.recordingHint}</p>
        <button
          type="button"
          onClick={stopRecording}
          className="mt-4 w-full rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3.5 text-base font-semibold text-red-300 hover:bg-red-500/20"
        >
          {t.voiceRecorder.stopRecording}
        </button>
      </div>
    );
  }

  if (state.kind === "transcribing") {
    return <p className="text-center text-sm text-slate-400">{t.voiceRecorder.transcribing}</p>;
  }

  return (
    <div>
      <p className="text-sm font-medium text-slate-300">{t.voiceRecorder.reviewHeading}</p>
      <p className="mt-1 text-sm text-slate-500">{t.voiceRecorder.reviewHint}</p>
      <textarea
        value={transcriptDraft}
        onInput={(event) => setTranscriptDraft((event.target as HTMLTextAreaElement).value)}
        rows={6}
        className="mt-3 w-full resize-none rounded-xl border border-white/[0.08] bg-white/[0.03] px-4 py-3 text-base text-white shadow-sm transition-all duration-300 focus:border-emerald-500/50 focus:bg-white/[0.05] focus:outline-none focus:ring-2 focus:ring-emerald-500/20"
      />
      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={reRecord}
          className="flex-1 rounded-xl border border-white/[0.08] px-4 py-2.5 text-sm font-medium text-slate-300 hover:bg-white/[0.05]"
        >
          {t.voiceRecorder.reRecord}
        </button>
        <button
          type="button"
          onClick={() => onConfirm(transcriptDraft)}
          disabled={!transcriptDraft.trim()}
          className="flex-1 rounded-xl bg-gradient-to-r from-emerald-600 to-emerald-700 px-4 py-2.5 text-sm font-semibold text-white shadow-lg shadow-emerald-500/20 transition-all duration-300 hover:from-emerald-500 hover:to-emerald-600 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {t.voiceRecorder.useThisText}
        </button>
      </div>
    </div>
  );
}
