import { useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Voice } from "./VoicesPage";

type Progress = { step: string; percent: number; message: string };
type Result = {
  output: string;
  bpm?: number;
  key?: string;
  duration?: number;
  seed?: number;
  seconds?: number;
  voice?: string;
};
type Format = "wav" | "mp3" | "flac";

const SUGGESTIONS = {
  song: [
    "Deutscher Pop, warme Akustikgitarre, Klavier, Männerstimme, gefühlvoll",
    "Energiegeladener Rock, E-Gitarren, Schlagzeug, kraftvoller Gesang",
    "Chillige Ballade, Klavier und Streicher, sanfte Frauenstimme",
    "Deutschrap, harter Beat, 808-Bass, selbstbewusst",
  ],
  instrumental: [
    "Lofi Hip-Hop, sanftes Klavier, Regen, entspannt",
    "Epische Filmmusik, Orchester, große Trommeln",
    "Elektronische Tanzmusik, treibender Bass, 128 BPM",
    "Akustische Gitarre, Lagerfeuer, warm und ruhig",
  ],
};

const LANGUAGES = [
  { id: "de", label: "Deutsch" },
  { id: "en", label: "Englisch" },
  { id: "tr", label: "Türkisch" },
  { id: "", label: "Automatisch" },
];

const KEYS = ["", "C major", "G major", "D major", "A major", "E major", "F major", "A minor", "E minor", "D minor", "C minor"];
const KEY_LABELS: Record<string, string> = { "": "Automatisch" };

function keyLabel(key: string) {
  return KEY_LABELS[key] ?? key.replace(" major", "-Dur").replace(" minor", "-Moll");
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

function time(seconds: number) {
  return `${Math.floor(seconds / 60)}:${String(Math.round(seconds % 60)).padStart(2, "0")}`;
}

function loadFormat(): Format {
  try {
    const saved = localStorage.getItem("tunify.format");
    return saved === "mp3" || saved === "flac" ? saved : "wav";
  } catch {
    return "wav";
  }
}

type Props = {
  /** „Instrumental erzeugen“ statt „Lied erzeugen“ */
  instrumental: boolean;
  /** Bleibt beim Seitenwechsel erhalten, damit ein laufendes Lied weiter entsteht. */
  active: boolean;
};

export default function SongPage({ instrumental, active }: Props) {
  const [caption, setCaption] = useState("");
  const [lyrics, setLyrics] = useState("");
  const [language, setLanguage] = useState("de");
  const [duration, setDuration] = useState(instrumental ? 30 : 90);
  const [voices, setVoices] = useState<Voice[]>([]);
  const [voice, setVoice] = useState<string>("");
  const [pitch, setPitch] = useState(0);
  const [bpm, setBpm] = useState("");
  const [key, setKey] = useState("");
  const [seed, setSeed] = useState("");
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cancelled, setCancelled] = useState(false);
  // Beide Seiten (Lied/Instrumental) hören dasselbe Ereignis – nur die, die gerade erzeugt, zeigt es an
  const busyRef = useRef(false);

  // Stimmen neu laden, sobald die Seite sichtbar wird (es könnten neue dazugekommen sein)
  useEffect(() => {
    if (!active || instrumental) return;
    invoke<Voice[]>("list_voices")
      .then(setVoices)
      .catch(() => setVoices([]));
  }, [active, instrumental]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    try {
      listen<Progress>("song-progress", (e) => {
        if (busyRef.current) setProgress(e.payload);
      })
        .then((fn) => (unlisten = fn))
        .catch(() => {});
    } catch {
      // außerhalb von Tauri gibt es keinen Fortschritt
    }
    return () => unlisten?.();
  }, []);

  async function generate(useSeed?: number) {
    setBusy(true);
    busyRef.current = true;
    setError(null);
    setCancelled(false);
    setResult(null);
    setProgress({ step: "start", percent: 0, message: "Motor bereit machen …" });
    try {
      setResult(
        await invoke<Result>("generate_song", {
          request: {
            caption,
            lyrics: instrumental ? "" : lyrics,
            instrumental,
            duration,
            bpm: bpm ? Number(bpm) : null,
            key,
            language: instrumental ? "" : language,
            seed: useSeed ?? (seed ? Number(seed) : null),
            voice: !instrumental && voice ? voice : null,
            pitch,
            format: loadFormat(),
          },
        }),
      );
    } catch (e) {
      if (String(e) === "Abgebrochen") setCancelled(true);
      else setError(String(e));
    } finally {
      setBusy(false);
      busyRef.current = false;
      setProgress(null);
    }
  }

  const ready = caption.trim() !== "" || (!instrumental && lyrics.trim() !== "");

  return (
    <div hidden={!active}>
      <h1>{instrumental ? "Instrumental erzeugen" : "Lied erzeugen"}</h1>
      <p className="muted">
        {instrumental
          ? "Beschreibe die Musik – Tunify erzeugt ein Instrumental ohne Gesang."
          : "Beschreibe den Stil und schreib einen Liedtext – Tunify erzeugt ein ganzes Lied mit Gesang, auf Wunsch in deiner Stimme."}
      </p>

      <section className="card wide">
        <h2>1 · Beschreibung</h2>
        <textarea
          aria-label="Beschreibung"
          rows={2}
          maxLength={500}
          placeholder={instrumental ? "z. B. Lofi Hip-Hop, sanftes Klavier, Regen" : "Stil, Instrumente, Stimmung, Gesang …"}
          value={caption}
          onChange={(e) => setCaption(e.target.value)}
          disabled={busy}
        />
        <div className="chips">
          {SUGGESTIONS[instrumental ? "instrumental" : "song"].map((s) => (
            <button key={s} className="ghost chip" onClick={() => setCaption(s)} disabled={busy}>
              {s}
            </button>
          ))}
        </div>
      </section>

      {!instrumental && (
        <section className="card wide">
          <h2>2 · Liedtext</h2>
          <textarea
            aria-label="Liedtext"
            rows={8}
            maxLength={4096}
            placeholder={"[Verse]\nErste Strophe …\n\n[Chorus]\nRefrain …"}
            value={lyrics}
            onChange={(e) => setLyrics(e.target.value)}
            disabled={busy}
          />
          <p className="muted small tip">
            Abschnitte mit [Verse], [Chorus], [Bridge] markieren. Ohne Text singt die Stimme ohne Worte.
          </p>
          <div className="field">
            <span>Sprache des Gesangs</span>
            <div className="segmented" role="radiogroup" aria-label="Sprache">
              {LANGUAGES.map((l) => (
                <button
                  key={l.id}
                  role="radio"
                  aria-checked={language === l.id}
                  className={language === l.id ? "active" : ""}
                  onClick={() => setLanguage(l.id)}
                  disabled={busy}
                >
                  {l.label}
                </button>
              ))}
            </div>
          </div>
          <div className="field">
            <label htmlFor="song-voice">Gesungen von</label>
            <select id="song-voice" value={voice} onChange={(e) => setVoice(e.target.value)} disabled={busy}>
              <option value="">KI-Stimme von ACE-Step</option>
              {voices.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                </option>
              ))}
            </select>
            {voice && (
              <div className="range-row">
                <span className="muted">Tonhöhe der Stimme {pitch > 0 ? `+${pitch}` : pitch}</span>
                <input
                  type="range"
                  min={-12}
                  max={12}
                  value={pitch}
                  aria-label="Tonhöhe der Stimme"
                  onChange={(e) => setPitch(Number(e.target.value))}
                  disabled={busy}
                />
              </div>
            )}
          </div>
        </section>
      )}

      <section className="card wide">
        <div className="card-head">
          <h2>{instrumental ? "2" : "3"} · Erzeugen</h2>
          <div className="actions">
            {busy && (
              <button className="ghost" onClick={() => invoke("cancel_conversion")}>
                Abbrechen
              </button>
            )}
            <button onClick={() => generate()} disabled={busy || !ready}>
              {busy ? "Entsteht …" : instrumental ? "Instrumental erzeugen" : "Lied erzeugen"}
            </button>
          </div>
        </div>
        <div className="field">
          <label htmlFor={`duration-${instrumental}`}>
            Länge <strong>{time(duration)}</strong>
          </label>
          <input
            id={`duration-${instrumental}`}
            type="range"
            min={instrumental ? 10 : 30}
            max={240}
            step={5}
            value={duration}
            onChange={(e) => setDuration(Number(e.target.value))}
            disabled={busy}
            className="full"
          />
        </div>
        <details className="advanced">
          <summary>Feineinstellungen</summary>
          <div className="advanced-grid">
            <label>
              Tempo (BPM)
              <input
                type="number"
                min={30}
                max={200}
                placeholder="automatisch"
                value={bpm}
                onChange={(e) => setBpm(e.target.value)}
                disabled={busy}
              />
            </label>
            <label>
              Tonart
              <select value={key} onChange={(e) => setKey(e.target.value)} disabled={busy}>
                {KEYS.map((k) => (
                  <option key={k} value={k}>
                    {keyLabel(k)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Zufallszahl
              <input
                type="number"
                placeholder="zufällig"
                value={seed}
                onChange={(e) => setSeed(e.target.value)}
                disabled={busy}
              />
            </label>
          </div>
          <p className="muted small tip">Gleiche Zufallszahl und gleiche Eingaben ergeben wieder dasselbe Stück.</p>
        </details>
        <p className="muted small tip">
          Beim ersten Mal lädt Tunify das Musikmodell (1–2 Minuten). Danach dauert eine Minute Musik etwa eine bis zwei
          Minuten{instrumental ? "" : ", in deiner Stimme etwas länger"}.
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
        {cancelled && <p className="muted">Abgebrochen.</p>}
        {result && (
          <div className="result" ref={(el) => el?.scrollIntoView({ behavior: "smooth", block: "end" })}>
            <p>
              <strong>{fileName(result.output)}</strong>{" "}
              <span className="muted">
                {result.bpm && `· ${result.bpm} BPM `}
                {result.key && `· ${keyLabel(result.key)} `}
                {result.voice && `· gesungen von ${result.voice} `}
                {result.seed !== undefined && `· Zufallszahl ${result.seed}`}
              </span>
            </p>
            <audio controls src={convertFileSrc(result.output)} />
            <div className="actions">
              <button className="ghost" onClick={() => revealItemInDir(result.output)}>
                Im Ordner zeigen
              </button>
              <button className="ghost" onClick={() => generate(-1)} disabled={busy}>
                Neue Variante
              </button>
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
