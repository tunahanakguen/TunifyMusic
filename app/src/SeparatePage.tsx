import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

type Progress = { step: string; percent: number; message: string };
type Result = { vocals: string; instrumental: string };
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

/** Gesang und Begleitung eines Liedes in zwei Dateien trennen (Karaoke, Remix, Cover). */
export default function SeparatePage() {
  const [input, setInput] = useState<string | null>(null);
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
  }

  async function pick() {
    const file = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Audio", extensions: AUDIO_EXTENSIONS }],
    });
    if (typeof file === "string") choose(file);
  }

  async function separate() {
    if (!input) return;
    setBusy(true);
    setError(null);
    setCancelled(false);
    setResult(null);
    setProgress({ step: "start", percent: 0, message: "Motor bereit machen …" });
    try {
      setResult(await invoke<Result>("separate_vocals", { request: { input, format } }));
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
      <h1>Gesang trennen</h1>
      <p className="muted">
        Ein Lied in Gesang und Begleitung aufteilen – für Karaoke, Remixe oder eigene Cover. Nur Lieder verwenden, an
        denen du die Rechte hast.
      </p>

      <section className="card wide">
        <h2>1 · Lied</h2>
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
      </section>

      <section className="card wide">
        <div className="card-head">
          <h2>2 · Trennen</h2>
          <div className="actions">
            {busy && (
              <button className="ghost" onClick={() => invoke("cancel_conversion")}>
                Abbrechen
              </button>
            )}
            <button onClick={separate} disabled={!input || busy}>
              {busy ? "Läuft …" : "Trennen"}
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
        {result &&
          [
            { label: "Gesang", path: result.vocals },
            { label: "Begleitung", path: result.instrumental },
          ].map((track) => (
            <div className="result" key={track.label}>
              <p>
                <strong>{track.label}</strong> <span className="muted">· {fileName(track.path)}</span>
              </p>
              <audio controls src={convertFileSrc(track.path)} />
            </div>
          ))}
        {result && (
          <div className="actions">
            <button className="ghost" onClick={() => revealItemInDir(result.vocals)}>
              Im Ordner zeigen
            </button>
          </div>
        )}
        <p className="muted small tip">Mehr Spuren (Schlagzeug, Bass …) trennt ein eigenes TUNAS-Programm.</p>
      </section>
    </>
  );
}
