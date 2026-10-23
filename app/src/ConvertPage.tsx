import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import YoutubeInput from "./YoutubeInput";
import Recorder from "./Recorder";
import { Voice } from "./VoicesPage";

type Progress = { step: string; percent: number; message: string };
type Result = { output: string; notes?: number; instrument?: string; octave?: number; midi?: string; voice?: string };
type Instrument = { id: string; label: string; available: boolean };
type Mode = "instrument" | "voice";
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

function isRecording(path: string) {
  return /[\\/]Tunify Music[\\/]Aufnahmen[\\/]/.test(path);
}

/** Kleine Einstellungen pro Gerät merken; ohne Speicher gelten die Vorgaben. */
function load<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const saved = localStorage.getItem(key);
    return allowed.includes(saved as T) ? (saved as T) : fallback;
  } catch {
    return fallback;
  }
}

function save(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // nur eine Bequemlichkeit
  }
}

function signed(n: number) {
  return n > 0 ? `+${n}` : String(n);
}

type Props = {
  initialInput: string | null;
  initialVoice: string | null;
  onOpenVoices: () => void;
};

export default function ConvertPage({ initialInput, initialVoice, onOpenVoices }: Props) {
  const [input, setInput] = useState<string | null>(initialInput);
  const [mode, setMode] = useState<Mode>(() =>
    initialVoice ? "voice" : load("tunify.mode", ["instrument", "voice"] as const, "instrument"),
  );
  // Instrument (Noten-Modus)
  const [instruments, setInstruments] = useState<Instrument[]>([{ id: "klavier", label: "Klavier", available: true }]);
  const [instrument, setInstrument] = useState("klavier");
  const [transpose, setTranspose] = useState(0);
  const [midi, setMidi] = useState(false);
  // Stimme (RVC)
  const [voices, setVoices] = useState<Voice[] | null>(null);
  const [voice, setVoice] = useState<string | null>(initialVoice);
  const [pitch, setPitch] = useState(0);
  const [formant, setFormant] = useState(1);
  const [indexRate, setIndexRate] = useState(0.3);
  const [protect, setProtect] = useState(0.33);
  // gemeinsam
  const [isolateVocals, setIsolateVocals] = useState(false);
  const [dereverb, setDereverb] = useState(false);
  const [denoise, setDenoise] = useState(false);
  const [mixInstrumental, setMixInstrumental] = useState(false);
  const [instrumentalPitch, setInstrumentalPitch] = useState(0);
  const [format, setFormat] = useState<Format>(() => load("tunify.format", ["wav", "mp3", "flac"] as const, "wav"));
  const [busy, setBusy] = useState(false);
  const [cancelled, setCancelled] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    invoke<Instrument[]>("list_instruments")
      .then(setInstruments)
      .catch(() => {});
    invoke<Voice[]>("list_voices")
      .then((list) => {
        setVoices(list);
        setVoice((current) => (current && list.some((v) => v.id === current) ? current : (list[0]?.id ?? null)));
      })
      .catch(() => setVoices([]));
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

  const selectedVoice = voices?.find((v) => v.id === voice) ?? null;
  const ready = !!input && (mode === "instrument" || !!selectedVoice);
  // Gleiche Tonart mit möglichst kleiner Verschiebung: Oktaven ändern die Tonart nicht (+7 klingt wie −5)
  const matchingKey = ((((mode === "voice" ? pitch : transpose) % 12) + 18) % 12) - 6;

  async function convert() {
    if (!input || !ready) return;
    setBusy(true);
    setError(null);
    setCancelled(false);
    setResult(null);
    setProgress({ step: "start", percent: 0, message: "Motor bereit machen …" });
    const mix = isolateVocals && mixInstrumental;
    const shared = { input, isolateVocals, dereverb, denoise, mixInstrumental: mix, instrumentalPitch: mix ? instrumentalPitch : 0, format };
    try {
      setResult(
        mode === "instrument"
          ? await invoke<Result>("convert_instrument", {
              request: {
                ...shared,
                instrument,
                label: instruments.find((i) => i.id === instrument)?.label ?? instrument,
                transpose,
                midi,
              },
            })
          : await invoke<Result>("convert_voice", {
              request: { ...shared, voice, pitch, formant, indexRate, protect },
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
    <>
      <h1>Stimme umwandeln</h1>
      <p className="muted">
        Sing oder summ eine Melodie – Tunify Music spielt sie auf einem echten Instrument nach oder singt sie mit einer
        anderen Stimme.
      </p>

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
        <YoutubeInput disabled={busy} onLoaded={choose} onError={setError} />
        {input && isRecording(input) && <audio className="preview" controls src={convertFileSrc(input)} />}
      </section>

      <section className="card wide">
        <div className="card-head">
          <h2>2 · Klang</h2>
          <div className="segmented" role="radiogroup" aria-label="Umwandeln in">
            {(
              [
                ["instrument", "Instrument"],
                ["voice", "Stimme"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                role="radio"
                aria-checked={mode === id}
                className={mode === id ? "active" : ""}
                disabled={busy}
                onClick={() => {
                  setMode(id);
                  save("tunify.mode", id);
                  setResult(null);
                }}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        {mode === "instrument" ? (
          <>
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
                Tonlage <strong>{signed(transpose)}</strong> Halbtöne
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
          </>
        ) : voices && voices.length === 0 ? (
          <div className="notice">
            <p>Noch keine Stimmen in deiner Bibliothek.</p>
            <button onClick={onOpenVoices}>Stimme importieren</button>
          </div>
        ) : (
          <>
            <div className="choices" role="radiogroup" aria-label="Stimme">
              {(voices ?? []).map((v) => (
                <button
                  key={v.id}
                  role="radio"
                  aria-checked={v.id === voice}
                  className={`choice ${v.id === voice ? "active" : ""}`}
                  disabled={busy}
                  onClick={() => setVoice(v.id)}
                >
                  {v.name}
                </button>
              ))}
              <button className="choice add" onClick={onOpenVoices} disabled={busy}>
                + Stimmen verwalten
              </button>
            </div>
            <p className="muted small tip">
              Funktioniert mit Gesang oder Sprache, nicht mit Instrumenten – für Instrumente oben „Instrument“ wählen.
            </p>
            <div className="field">
              <label htmlFor="pitch">
                Tonhöhe <strong>{signed(pitch)}</strong> Halbtöne
                <span className="muted"> · Männer- zu Frauenstimme etwa +12, umgekehrt −12</span>
              </label>
              <div className="range-row">
                <button className="ghost" onClick={() => setPitch(Math.max(-12, pitch - 12))} disabled={busy}>
                  − Oktave
                </button>
                <input
                  id="pitch"
                  type="range"
                  min={-12}
                  max={12}
                  value={pitch}
                  onChange={(e) => setPitch(Number(e.target.value))}
                  disabled={busy}
                />
                <button className="ghost" onClick={() => setPitch(Math.min(12, pitch + 12))} disabled={busy}>
                  + Oktave
                </button>
              </div>
            </div>
            <div className="field">
              <label htmlFor="formant">
                Klangfarbe <strong>{formant.toFixed(2).replace(".", ",")}</strong>
                <span className="muted"> · unter 1 dunkler und größer, über 1 heller und kleiner</span>
              </label>
              <div className="range-row">
                <input
                  id="formant"
                  type="range"
                  min={0.5}
                  max={2}
                  step={0.05}
                  value={formant}
                  onChange={(e) => setFormant(Number(e.target.value))}
                  disabled={busy}
                />
                <button className="ghost" onClick={() => setFormant(1)} disabled={busy || formant === 1}>
                  Zurücksetzen
                </button>
              </div>
            </div>
            <details className="advanced">
              <summary>Feineinstellungen</summary>
              <div className="field">
                <label htmlFor="index-rate">
                  Stimm-Ähnlichkeit (Index) <strong>{Math.round(indexRate * 100)} %</strong>
                  <span className="muted">
                    {selectedVoice?.index ? " · höher klingt mehr nach der Zielstimme, aber weniger deutlich" : " · diese Stimme hat keinen Index"}
                  </span>
                </label>
                <input
                  id="index-rate"
                  type="range"
                  min={0}
                  max={1}
                  step={0.05}
                  value={indexRate}
                  onChange={(e) => setIndexRate(Number(e.target.value))}
                  disabled={busy || !selectedVoice?.index}
                />
              </div>
              <div className="field">
                <label htmlFor="protect">
                  Atmen und Konsonanten schützen <strong>{Math.round(protect * 100)} %</strong>
                  <span className="muted"> · höher gegen verwaschene S- und Atemlaute, 50 % = aus</span>
                </label>
                <input
                  id="protect"
                  type="range"
                  min={0}
                  max={0.5}
                  step={0.01}
                  value={protect}
                  onChange={(e) => setProtect(Number(e.target.value))}
                  disabled={busy}
                />
              </div>
            </details>
          </>
        )}

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
        {isolateVocals && mixInstrumental && (
          <div className="field">
            <label htmlFor="instrumental-pitch">
              Tonhöhe der Begleitung <strong>{signed(instrumentalPitch)}</strong> Halbtöne
              <span className="muted"> · damit Begleitung und neue Stimme in derselben Tonart sind</span>
            </label>
            <div className="range-row">
              <input
                id="instrumental-pitch"
                type="range"
                min={-12}
                max={12}
                value={instrumentalPitch}
                onChange={(e) => setInstrumentalPitch(Number(e.target.value))}
                disabled={busy}
              />
              <button
                className="ghost"
                onClick={() => setInstrumentalPitch(matchingKey)}
                disabled={busy || instrumentalPitch === matchingKey}
              >
                Passend {mode === "voice" ? "zur Stimme" : "zum Instrument"} ({signed(matchingKey)})
              </button>
            </div>
          </div>
        )}
      </section>

      <section className="card wide">
        <div className="card-head">
          <h2>3 · Umwandeln</h2>
          <div className="actions">
            {busy && (
              <button className="ghost" onClick={() => invoke("cancel_conversion")}>
                Abbrechen
              </button>
            )}
            <button onClick={convert} disabled={!ready || busy}>
              {busy ? "Läuft …" : "Umwandeln"}
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
                onClick={() => {
                  setFormat(f.id);
                  save("tunify.format", f.id);
                }}
                disabled={busy}
              >
                {f.label}
              </button>
            ))}
          </div>
          {mode === "instrument" && (
            <label>
              <input type="checkbox" checked={midi} onChange={(e) => setMidi(e.target.checked)} disabled={busy} />
              Noten zusätzlich als MIDI speichern
            </label>
          )}
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
          <div className="result" ref={(el) => el?.scrollIntoView({ behavior: "smooth", block: "end" })}>
            <p>
              <strong>{fileName(result.output)}</strong>{" "}
              <span className="muted">
                {result.voice && `· gesungen von ${result.voice}`}
                {result.notes !== undefined && `· ${result.notes} Töne erkannt`}
                {!!result.octave &&
                  ` · automatisch ${Math.abs(result.octave)} Oktave${Math.abs(result.octave) > 1 ? "n" : ""} ${
                    result.octave > 0 ? "höher" : "tiefer"
                  } (Lage ${result.instrument})`}
              </span>
            </p>
            <audio controls src={convertFileSrc(result.output)} />
            <div className="actions">
              <button className="ghost" onClick={() => revealItemInDir(result.output)}>
                Im Ordner zeigen
              </button>
              {result.midi && <span className="muted">Noten gespeichert: {fileName(result.midi)}</span>}
            </div>
          </div>
        )}
      </section>
    </>
  );
}
