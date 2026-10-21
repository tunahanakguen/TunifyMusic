import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import Recorder from "./Recorder";

type Progress = { step: string; percent: number; message: string };
type Result = { output: string; notes: number; instrument: string; octave: number };
type Instrument = { id: string; label: string; available: boolean };

const AUDIO_EXTENSIONS = ["mp3", "wav", "m4a", "flac", "ogg", "aac", "opus", "webm", "mp4"];

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

function isRecording(path: string) {
  return /[\\/]Tunify Music[\\/]Aufnahmen[\\/]/.test(path);
}

export default function ConvertPage() {
  const [input, setInput] = useState<string | null>(null);
  const [instruments, setInstruments] = useState<Instrument[]>([{ id: "klavier", label: "Klavier", available: true }]);
  const [instrument, setInstrument] = useState("klavier");
  const [transpose, setTranspose] = useState(0);
  const [isolateVocals, setIsolateVocals] = useState(false);
  const [dereverb, setDereverb] = useState(false);
  const [denoise, setDenoise] = useState(false);
  const [mixInstrumental, setMixInstrumental] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    invoke<Instrument[]>("list_instruments")
      .then(setInstruments)
      .catch(() => {});
  }, []);

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenDrop: (() => void) | undefined;
    try {
      listen<Progress>("engine-progress", (e) => setProgress(e.payload))
        .then((fn) => (unlistenProgress = fn))
        .catch(() => {});
      getCurrentWebview()
        .onDragDropEvent((e) => {
          if (e.payload.type === "over" || e.payload.type === "enter") setDragging(true);
          else if (e.payload.type === "leave") setDragging(false);
          else if (e.payload.type === "drop") {
            setDragging(false);
            const file = e.payload.paths.find((p) =>
              AUDIO_EXTENSIONS.includes(p.split(".").pop()?.toLowerCase() ?? ""),
            );
            if (file) choose(file);
          }
        })
        .then((fn) => (unlistenDrop = fn));
    } catch {
      // außerhalb von Tauri (reiner Browser) gibt es weder Fortschritt noch Ziehen und Ablegen
    }
    return () => {
      unlistenProgress?.();
      unlistenDrop?.();
    };
  }, []);

  function choose(path: string) {
    setInput(path);
    setResult(null);
    setError(null);
  }

  async function pick() {
    const file = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Audio", extensions: AUDIO_EXTENSIONS }],
    });
    if (typeof file === "string") choose(file);
  }

  async function convert() {
    if (!input) return;
    setBusy(true);
    setError(null);
    setResult(null);
    setProgress({ step: "start", percent: 0, message: "Motor starten" });
    try {
      setResult(
        await invoke<Result>("convert_instrument", {
          request: {
            input,
            instrument,
            label: instruments.find((i) => i.id === instrument)?.label ?? instrument,
            transpose,
            isolateVocals,
            dereverb,
            denoise,
            mixInstrumental: isolateVocals && mixInstrumental,
          },
        }),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  return (
    <>
      <h1>Stimme umwandeln</h1>
      <p className="muted">Sing oder summ eine Melodie – Tunify Music spielt sie auf einem echten Instrument nach.</p>

      <section className="card wide">
        <h2>1 · Aufnahme</h2>
        <div className="input-row">
          <button className={`dropzone ${dragging ? "over" : ""}`} onClick={pick} disabled={busy}>
            {input ? (
              <>
                <strong>{fileName(input)}</strong>
                <span className="muted">Andere Datei wählen oder hierher ziehen</span>
              </>
            ) : (
              <>
                <strong>Audiodatei wählen</strong>
                <span className="muted">oder hierher ziehen · MP3, WAV, M4A, FLAC …</span>
              </>
            )}
          </button>
          <Recorder disabled={busy} onRecorded={choose} onError={setError} />
        </div>
        {input && isRecording(input) && <audio className="preview" controls src={convertFileSrc(input)} />}
      </section>

      <section className="card wide">
        <h2>2 · Klang</h2>
        <div className="choices" role="radiogroup" aria-label="Instrument">
          {instruments.map((i) => (
            <button
              key={i.id}
              role="radio"
              aria-checked={i.id === instrument}
              className={`choice ${i.id === instrument ? "active" : ""}`}
              disabled={!i.available || busy}
              title={i.available ? undefined : "Klangsatz fehlt – tools\\setup-instruments.ps1 ausführen"}
              onClick={() => setInstrument(i.id)}
            >
              {i.label}
            </button>
          ))}
        </div>

        <div className="field">
          <label htmlFor="transpose">
            Tonlage <strong>{transpose > 0 ? `+${transpose}` : transpose}</strong> Halbtöne
            <span className="muted"> · zusätzlich legt Tunify die Melodie automatisch in die Lage des Instruments</span>
          </label>
          <div className="range-row">
            <button className="ghost" onClick={() => setTranspose(Math.max(-24, transpose - 12))} disabled={busy}>
              − Oktave
            </button>
            <input
              id="transpose"
              type="range"
              min={-24}
              max={24}
              value={transpose}
              onChange={(e) => setTranspose(Number(e.target.value))}
              disabled={busy}
            />
            <button className="ghost" onClick={() => setTranspose(Math.min(24, transpose + 12))} disabled={busy}>
              + Oktave
            </button>
          </div>
        </div>

        <div className="toggles">
          <label>
            <input type="checkbox" checked={isolateVocals} onChange={(e) => setIsolateVocals(e.target.checked)} disabled={busy} />
            Begleitmusik entfernen <span className="muted">(für Lieder mit Musik)</span>
          </label>
          <label>
            <input type="checkbox" checked={dereverb} onChange={(e) => setDereverb(e.target.checked)} disabled={busy} />
            Hall und Echo entfernen
          </label>
          <label>
            <input type="checkbox" checked={denoise} onChange={(e) => setDenoise(e.target.checked)} disabled={busy} />
            Rauschen reduzieren
          </label>
          <label className={isolateVocals ? "" : "off"}>
            <input
              type="checkbox"
              checked={isolateVocals && mixInstrumental}
              onChange={(e) => setMixInstrumental(e.target.checked)}
              disabled={busy || !isolateVocals}
            />
            Begleitmusik wieder dazumischen
          </label>
        </div>
      </section>

      <section className="card wide">
        <div className="card-head">
          <h2>3 · Umwandeln</h2>
          <button onClick={convert} disabled={!input || busy}>
            {busy ? "Läuft …" : "Umwandeln"}
          </button>
        </div>
        {progress && (
          <div className="progress">
            <div className="bar">
              <div style={{ width: `${progress.percent}%` }} />
            </div>
            <span className="muted">{progress.message}</span>
          </div>
        )}
        {error && <p className="error">{error}</p>}
        {result && (
          <div className="result" ref={(el) => el?.scrollIntoView({ behavior: "smooth", block: "end" })}>
            <p>
              <strong>{fileName(result.output)}</strong> <span className="muted">
                · {result.notes} Töne erkannt
                {result.octave !== 0 &&
                  ` · automatisch ${Math.abs(result.octave)} Oktave${Math.abs(result.octave) > 1 ? "n" : ""} ${
                    result.octave > 0 ? "höher" : "tiefer"
                  } (Lage ${result.instrument})`}
              </span>
            </p>
            <audio controls src={convertFileSrc(result.output)} />
            <button className="ghost" onClick={() => revealItemInDir(result.output)}>
              Im Ordner zeigen
            </button>
          </div>
        )}
      </section>
    </>
  );
}
