import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import Recorder from "./Recorder";
import { Voice } from "./VoicesPage";

type Progress = { step: string; percent: number; message: string };
type Rating = { path: string; grade: "gut" | "brauchbar" | "schlecht"; hints: string[]; seconds: number };

const AUDIO_EXTENSIONS = ["mp3", "wav", "m4a", "flac", "ogg", "aac", "opus", "webm", "mp4"];

const PRESETS = [
  { epochs: 100, label: "Schnell", hint: "zum Ausprobieren" },
  { epochs: 200, label: "Standard", hint: "wie Musicfy" },
  { epochs: 400, label: "Gründlich", hint: "für viel Material" },
];

function minutes(seconds: number) {
  return `${Math.floor(seconds / 60)}:${String(Math.round(seconds % 60)).padStart(2, "0")}`;
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

type Props = {
  /** Die Seite bleibt beim Wechseln im Hintergrund erhalten, damit ein Training weiterläuft. */
  active: boolean;
  onUse: (voiceId: string) => void;
};

export default function TrainPage({ active, onUse }: Props) {
  const [name, setName] = useState("");
  const [inputs, setInputs] = useState<string[]>([]);
  const [epochs, setEpochs] = useState(200);
  const [isolateVocals, setIsolateVocals] = useState(false);
  const [dereverb, setDereverb] = useState(false);
  const [denoise, setDenoise] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cancelled, setCancelled] = useState(false);
  const [trained, setTrained] = useState<Voice | null>(null);
  const [dragging, setDragging] = useState(false);
  // Bewertung je Datei; fehlt ein Eintrag, wird die Datei noch geprüft
  const [ratings, setRatings] = useState<Record<string, Rating>>({});
  const activeRef = useRef(active);
  activeRef.current = active;

  function add(paths: string[]) {
    setInputs((current) => [...current, ...paths.filter((p) => !current.includes(p))]);
    setTrained(null);
    setError(null);
    const fresh = paths.filter((p) => !(p in ratings));
    if (fresh.length === 0) return;
    invoke<Rating[]>("analyze_recordings", { files: fresh })
      .then((list) => setRatings((current) => ({ ...current, ...Object.fromEntries(list.map((r, i) => [fresh[i], r])) })))
      .catch(() => {});
  }

  const rated = inputs.map((p) => ratings[p]).filter(Boolean);
  const totalSeconds = rated.reduce((sum, r) => sum + r.seconds, 0);

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenDrop: (() => void) | undefined;
    try {
      listen<Progress>("train-progress", (e) => setProgress(e.payload))
        .then((fn) => (unlistenProgress = fn))
        .catch(() => {});
      getCurrentWebview()
        .onDragDropEvent((e) => {
          if (!activeRef.current) return; // Ablegen gilt nur, solange diese Seite sichtbar ist
          if (e.payload.type === "over" || e.payload.type === "enter") setDragging(true);
          else if (e.payload.type === "leave") setDragging(false);
          else if (e.payload.type === "drop") {
            setDragging(false);
            add(e.payload.paths.filter((p) => AUDIO_EXTENSIONS.includes(p.split(".").pop()?.toLowerCase() ?? "")));
          }
        })
        .then((fn) => (unlistenDrop = fn));
    } catch {
      // außerhalb von Tauri gibt es weder Fortschritt noch Ziehen und Ablegen
    }
    return () => {
      unlistenProgress?.();
      unlistenDrop?.();
    };
  }, []);

  async function pick() {
    const files = await open({
      multiple: true,
      directory: false,
      filters: [{ name: "Audio", extensions: AUDIO_EXTENSIONS }],
    });
    if (files) add(files);
  }

  async function start() {
    setBusy(true);
    setError(null);
    setCancelled(false);
    setTrained(null);
    setProgress({ step: "start", percent: 0, message: "Motor bereit machen …" });
    try {
      setTrained(
        await invoke<Voice>("train_voice", {
          request: { name, inputs, epochs, isolateVocals, dereverb, denoise },
        }),
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
    <div hidden={!active}>
      <h1>Stimme trainieren</h1>
      <p className="muted">
        Aus Aufnahmen deiner Stimme ein eigenes Stimm-Modell machen. Danach kannst du unter „Stimme umwandeln“ jede
        Melodie damit singen lassen.
      </p>

      <section className="card wide">
        <h2>1 · Name</h2>
        <input
          type="text"
          aria-label="Name der Stimme"
          placeholder="z. B. Meine Stimme"
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={busy}
        />
      </section>

      <section className="card wide">
        <h2>2 · Aufnahmen</h2>
        <div className="input-row">
          <button className={`dropzone ${dragging ? "over" : ""}`} onClick={pick} disabled={busy}>
            <strong>Dateien hinzufügen</strong>
            <span className="muted">oder hierher ziehen · mehrere auf einmal möglich</span>
          </button>
          <Recorder disabled={busy} onRecorded={(path) => add([path])} onError={setError} />
        </div>
        {inputs.length > 0 && (
          <ul className="file-list train-files">
            {inputs.map((path) => (
              <li key={path}>
                <div className="train-file">
                  <span>
                    {fileName(path)}
                    {ratings[path] && <span className="muted"> · {minutes(ratings[path].seconds)}</span>}
                  </span>
                  {ratings[path] ? (
                    <span className="muted small">
                      <span className={`grade ${ratings[path].grade}`}>{ratings[path].grade}</span>
                      {ratings[path].hints.length > 0 && ` ${ratings[path].hints.join(" · ")}`}
                    </span>
                  ) : (
                    <span className="muted small">wird geprüft …</span>
                  )}
                </div>
                <button
                  className="ghost small-button"
                  aria-label={`${fileName(path)} entfernen`}
                  onClick={() => setInputs(inputs.filter((p) => p !== path))}
                  disabled={busy}
                >
                  Entfernen
                </button>
              </li>
            ))}
          </ul>
        )}
        {rated.length > 0 && (
          <p className={`train-total ${totalSeconds < 30 ? "error" : ""}`}>
            Zusammen <strong>{minutes(totalSeconds)} min</strong>
            <span className="muted">
              {totalSeconds < 30
                ? " · zu wenig, mindestens 30 Sekunden"
                : totalSeconds < 600
                  ? " · reicht zum Ausprobieren, für eine gute Stimme 10–30 Minuten"
                  : " · gute Menge"}
            </span>
          </p>
        )}
        <p className="muted small tip">
          Am besten 10–30 Minuten nur Gesang, ohne Musik und Hall, in verschiedenen Tonhöhen. Ab 30 Sekunden geht es,
          klingt dann aber noch ungenau. Nur Aufnahmen verwenden, an denen du die Rechte hast.
        </p>
        <div className="toggles">
          <label>
            <input type="checkbox" checked={isolateVocals} onChange={(e) => setIsolateVocals(e.target.checked)} disabled={busy} />
            Begleitmusik entfernen <span className="muted">(falls in den Aufnahmen Musik läuft)</span>
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
      </section>

      <section className="card wide">
        <div className="card-head">
          <h2>3 · Trainieren</h2>
          <div className="actions">
            {busy && (
              <button className="ghost" onClick={() => invoke("cancel_conversion")}>
                Abbrechen
              </button>
            )}
            <button onClick={start} disabled={busy || !name.trim() || inputs.length === 0}>
              {busy ? "Trainiert …" : "Training starten"}
            </button>
          </div>
        </div>
        <div className="choices" role="radiogroup" aria-label="Gründlichkeit">
          {PRESETS.map((p) => (
            <button
              key={p.epochs}
              role="radio"
              aria-checked={epochs === p.epochs}
              className={`choice ${epochs === p.epochs ? "active" : ""}`}
              onClick={() => setEpochs(p.epochs)}
              disabled={busy}
            >
              {p.label}
              <small>
                {p.epochs} Epochen · {p.hint}
              </small>
            </button>
          ))}
        </div>
        <p className="muted small tip">
          Die Dauer hängt von der Menge ab: 1–2 Minuten Gesang brauchen bei „Standard“ etwa 20 Minuten, 10 Minuten etwa
          40 Minuten. Du kannst währenddessen andere Seiten öffnen – nur Umwandeln geht erst danach.
        </p>
        {progress && (
          <div className="progress">
            <div className="bar">
              <div style={{ width: `${progress.percent}%` }} />
            </div>
            <span className="muted">{progress.message}</span>
          </div>
        )}
        {error && <p className="error">{error}</p>}
        {cancelled && <p className="muted">Abgebrochen. Die Zwischenstände wurden gelöscht.</p>}
        {trained && (
          <div className="notice done">
            <p>
              <strong>„{trained.name}“</strong> ist fertig und liegt in deiner Bibliothek.
            </p>
            <button onClick={() => onUse(trained.id)}>Jetzt benutzen</button>
          </div>
        )}
      </section>
    </div>
  );
}
