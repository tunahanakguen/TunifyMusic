import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import YoutubeInput from "./YoutubeInput";

type Progress = { step: string; percent: number; message: string };
type Result = { output: string };
type Format = "wav" | "mp3" | "flac";

const AUDIO_EXTENSIONS = ["mp3", "wav", "m4a", "flac", "ogg", "aac", "opus", "webm", "mp4"];
const FORMATS: { id: Format; label: string }[] = [
  { id: "wav", label: "WAV" },
  { id: "mp3", label: "MP3" },
  { id: "flac", label: "FLAC" },
];

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Audio bereinigen: Begleitmusik, Hall/Echo und Rauschen entfernen – ohne Stimmwechsel, eine Datei als Ergebnis. */
export default function CleanPage() {
  const [input, setInput] = useState<string | null>(null);
  const [isolateVocals, setIsolateVocals] = useState(false);
  const [dereverb, setDereverb] = useState(true);
  const [denoise, setDenoise] = useState(true);
  const [format, setFormat] = useState<Format>("wav");
  const [busy, setBusy] = useState(false);
  const [cancelled, setCancelled] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);

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
      // außerhalb von Tauri
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
    setProgress(null);
  }

  async function pick() {
    const file = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Audio", extensions: AUDIO_EXTENSIONS }],
    });
    if (typeof file === "string") choose(file);
  }

  const anyStep = isolateVocals || dereverb || denoise;

  async function clean() {
    if (!input || !anyStep) return;
    setBusy(true);
    setError(null);
    setCancelled(false);
    setResult(null);
    setProgress({ step: "start", percent: 0, message: "Motor bereit machen …" });
    try {
      setResult(
        await invoke<Result>("clean_audio", { request: { input, isolateVocals, dereverb, denoise, format } }),
      );
    } catch (e) {
      if (String(e) === "Abgebrochen") setCancelled(true);
      else setError(String(e));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  }

  return (
    <>
      <h1>Audio bereinigen</h1>
      <p className="muted">
        Eine Aufnahme oder ein Lied säubern, ohne die Stimme zu verändern: Begleitmusik, Hall und Echo oder Rauschen
        entfernen.
      </p>

      <section className="card wide">
        <h2>1 · Datei</h2>
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
        <YoutubeInput disabled={busy} onLoaded={choose} onError={setError} />
      </section>

      <section className="card wide">
        <h2>2 · Was soll weg?</h2>
        <div className="toggles">
          <label>
            <input type="checkbox" checked={isolateVocals} onChange={(e) => setIsolateVocals(e.target.checked)} disabled={busy} />
            Begleitmusik entfernen <span className="muted">(nur Gesang/Sprache bleibt)</span>
          </label>
          <label>
            <input type="checkbox" checked={dereverb} onChange={(e) => setDereverb(e.target.checked)} disabled={busy} />
            Hall und Echo entfernen
          </label>
          <label>
            <input type="checkbox" checked={denoise} onChange={(e) => setDenoise(e.target.checked)} disabled={busy} />
            Rauschen reduzieren
          </label>
        </div>
        {!anyStep && <p className="muted small">Bitte mindestens einen Schritt wählen.</p>}
      </section>

      <section className="card wide">
        <div className="card-head">
          <h2>3 · Bereinigen</h2>
          <div className="actions">
            {busy && (
              <button className="ghost" onClick={() => invoke("cancel_conversion")}>
                Abbrechen
              </button>
            )}
            <button onClick={clean} disabled={!input || !anyStep || busy}>
              {busy ? "Läuft …" : "Bereinigen"}
            </button>
          </div>
        </div>
        <div className="output-options">
          <div className="segmented" role="radiogroup" aria-label="Format">
            {FORMATS.map((f) => (
              <button
                key={f.id}
                role="radio"
                aria-checked={format === f.id}
                className={format === f.id ? "active" : ""}
                onClick={() => setFormat(f.id)}
                disabled={busy}
              >
                {f.label}
              </button>
            ))}
          </div>
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
        {cancelled && <p className="muted">Abgebrochen.</p>}
        {result && (
          <div className="result">
            <p>
              <strong>{fileName(result.output)}</strong>
            </p>
            <audio controls src={convertFileSrc(result.output)} />
            <div className="actions">
              <button className="ghost" onClick={() => revealItemInDir(result.output)}>
                Im Ordner zeigen
              </button>
            </div>
          </div>
        )}
      </section>
    </>
  );
}
