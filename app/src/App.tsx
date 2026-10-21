import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ThemeChoice, useTheme } from "./theme";
import ConvertPage from "./ConvertPage";
import HistoryPage from "./HistoryPage";
import "./App.css";

type EngineInfo = {
  engine: string;
  python: string;
  torch: string;
  cuda: string | null;
  gpu: { name: string; vram_gb: number; compute_capability: string } | null;
};

const AREAS = [
  { id: "start", label: "Start", phase: "" },
  { id: "convert", label: "Stimme umwandeln", phase: "" },
  { id: "voices", label: "Stimmen", phase: "Phase 3" },
  { id: "train", label: "Stimme trainieren", phase: "Phase 4" },
  { id: "song", label: "Lied erzeugen", phase: "Phase 5" },
  { id: "instrumental", label: "Instrumental erzeugen", phase: "Phase 5" },
  { id: "history", label: "Verlauf", phase: "" },
];

function EngineStatus() {
  const [info, setInfo] = useState<EngineInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  async function check() {
    setLoading(true);
    setError(null);
    try {
      setInfo(await invoke<EngineInfo>("engine_info"));
    } catch (e) {
      setInfo(null);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    check();
  }, []);

  return (
    <section className="card">
      <div className="card-head">
        <h2>
          <span className={`status-dot ${info ? "ok" : error ? "fail" : ""}`} />
          KI-Motor
        </h2>
        <button onClick={check} disabled={loading}>
          {loading ? "Prüfe …" : "Neu prüfen"}
        </button>
      </div>
      {loading && <p className="muted">Python-Motor wird gestartet …</p>}
      {error && <p className="error">{error}</p>}
      {info && (
        <dl>
          <dt>Grafikkarte</dt>
          <dd>{info.gpu ? `${info.gpu.name} · ${info.gpu.vram_gb} GB` : "keine CUDA-Grafikkarte gefunden"}</dd>
          <dt>PyTorch</dt>
          <dd>
            {info.torch} {info.cuda ? `(CUDA ${info.cuda})` : "(nur CPU)"}
          </dd>
          <dt>Python</dt>
          <dd>{info.python}</dd>
          <dt>Motor</dt>
          <dd>{info.engine}</dd>
        </dl>
      )}
    </section>
  );
}

const THEMES: { id: ThemeChoice; label: string }[] = [
  { id: "light", label: "Hell" },
  { id: "dark", label: "Dunkel" },
  { id: "system", label: "Wie Windows" },
];

function ThemeSwitch() {
  const { choice, setChoice } = useTheme();
  return (
    <div className="theme-switch" role="radiogroup" aria-label="Design">
      {THEMES.map((t) => (
        <button
          key={t.id}
          role="radio"
          aria-checked={choice === t.id}
          className={choice === t.id ? "active" : ""}
          onClick={() => setChoice(t.id)}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

export default function App() {
  const [area, setArea] = useState("start");
  // aus dem Verlauf heraus „Umwandeln“: Datei vorwählen; der Zähler erzwingt eine frische Seite
  const [handoff, setHandoff] = useState<{ path: string; n: number } | null>(null);
  return (
    <div className="shell">
      <header className="topbar">
        <img src="/tunify-logo.png" alt="TUNAS" className="logo" />
        <span className="brand">
          Tunify <em>Music</em>
        </span>
        <ThemeSwitch />
      </header>
      <nav className="sidebar">
        {AREAS.map((a) => (
          <button
            key={a.id}
            className={`nav-item ${a.id === area ? "active" : ""}`}
            disabled={!!a.phase}
            aria-current={a.id === area ? "page" : undefined}
            title={a.phase ? `kommt in ${a.phase}` : undefined}
            onClick={() => {
              setArea(a.id);
              setHandoff(null);
            }}
          >
            <span>{a.label}</span>
            <small>{a.phase}</small>
          </button>
        ))}
      </nav>
      <main className="content">
        {area === "convert" ? (
          <ConvertPage key={handoff?.n ?? 0} initialInput={handoff?.path ?? null} />
        ) : area === "history" ? (
          <HistoryPage
            onConvert={(path) => {
              setHandoff({ path, n: (handoff?.n ?? 0) + 1 });
              setArea("convert");
            }}
          />
        ) : (
          <>
            <h1>Willkommen</h1>
            <p className="muted">
              Gesang und Melodien lokal umwandeln, Stimmen trainieren und Musik erzeugen – ohne Konto, ohne Limit.
            </p>
            <button onClick={() => setArea("convert")}>Stimme umwandeln</button>
            <EngineStatus />
          </>
        )}
      </main>
    </div>
  );
}
