import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Props = {
  disabled: boolean;
  onRecorded: (path: string) => void;
  onError: (message: string) => void;
};

/** Mikrofon-Aufnahme direkt in der App. Speichert über Rust als WAV unter Musik\Tunify Music\Aufnahmen. */
export default function Recorder({ disabled, onRecorded, onError }: Props) {
  const [waiting, setWaiting] = useState(false);
  const [recording, setRecording] = useState(false);
  const [saving, setSaving] = useState(false);
  const [seconds, setSeconds] = useState(0);
  const [level, setLevel] = useState(0);
  const stop = useRef<(() => void) | null>(null);

  useEffect(() => () => stop.current?.(), []);

  async function start() {
    let stream: MediaStream;
    setWaiting(true);
    try {
      // Für Gesang keine Sprach-Filter: die verfälschen Tonhöhe und Lautstärke
      stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false },
      });
    } catch (e) {
      onError(`Kein Zugriff aufs Mikrofon: ${e instanceof Error ? e.message : e}`);
      return;
    } finally {
      setWaiting(false);
    }

    const recorder = new MediaRecorder(stream, { mimeType: "audio/webm;codecs=opus" });
    const chunks: Blob[] = [];
    recorder.ondataavailable = (e) => e.data.size && chunks.push(e.data);

    const context = new AudioContext();
    const analyser = context.createAnalyser();
    analyser.fftSize = 1024;
    context.createMediaStreamSource(stream).connect(analyser);
    const samples = new Float32Array(analyser.fftSize);
    let frame = 0;
    const meter = () => {
      analyser.getFloatTimeDomainData(samples);
      const peak = samples.reduce((max, s) => Math.max(max, Math.abs(s)), 0);
      setLevel(peak);
      frame = requestAnimationFrame(meter);
    };
    meter();

    const began = Date.now();
    const timer = window.setInterval(() => setSeconds(Math.floor((Date.now() - began) / 1000)), 250);

    const cleanup = () => {
      cancelAnimationFrame(frame);
      clearInterval(timer);
      stream.getTracks().forEach((t) => t.stop());
      context.close();
      stop.current = null;
      setRecording(false);
      setLevel(0);
    };

    recorder.onstop = async () => {
      cleanup();
      setSaving(true);
      try {
        const bytes = new Uint8Array(await new Blob(chunks, { type: "audio/webm" }).arrayBuffer());
        onRecorded(await invoke<string>("save_recording", bytes));
      } catch (e) {
        onError(String(e));
      } finally {
        setSaving(false);
      }
    };

    stop.current = () => recorder.state !== "inactive" && recorder.stop();
    recorder.start(250);
    setSeconds(0);
    setRecording(true);
  }

  const time = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;

  return (
    <div className={`recorder ${recording ? "on" : ""}`}>
      {recording ? (
        <button className="record stop" onClick={() => stop.current?.()}>
          <span className="icon" aria-hidden="true" />
          Stopp · {time}
        </button>
      ) : (
        <button className="record" onClick={start} disabled={disabled || saving || waiting}>
          <span className="icon" aria-hidden="true" />
          {saving ? "Speichere …" : waiting ? "Warte auf Mikrofon …" : "Aufnehmen"}
        </button>
      )}
      <div className={`level ${level > 0.98 ? "clip" : ""}`} aria-hidden="true">
        <div style={{ width: `${Math.min(100, level * 100)}%` }} />
      </div>
      <span className="muted">
        {recording
          ? level > 0.98
            ? "Zu laut – etwas Abstand zum Mikrofon"
            : "Sing oder summ jetzt …"
          : "Mit dem Mikrofon singen oder summen"}
      </span>
    </div>
  );
}
