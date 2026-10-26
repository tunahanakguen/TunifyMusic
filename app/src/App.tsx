import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ThemeChoice, useTheme } from "./theme";
import ConvertPage from "./ConvertPage";
import HistoryPage from "./HistoryPage";
import SeparatePage from "./SeparatePage";
import CleanPage from "./CleanPage";
import VoicesPage from "./VoicesPage";
import TrainPage from "./TrainPage";
import SongPage from "./SongPage";
import SetupPanel, { SetupStatus } from "./SetupPanel";
import UpdateBanner from "./UpdateBanner";
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
  { id: "separate", label: "Gesang trennen", phase: "" },
  { id: "clean", label: "Audio bereinigen", phase: "" },
  { id: "voices", label: "Stimmen", phase: "" },
  { id: "train", label: "Stimme trainieren", phase: "" },
  { id: "song", label: "Lied erzeugen", phase: "" },
  { id: "instrumental", label: "Instrumental erzeugen", phase: "" },
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
  // aus Verlauf oder Stimmen heraus „Umwandeln“/„Benutzen“: Datei bzw. Stimme vorwählen;
  // der Zähler erzwingt eine frische Seite
  const [handoff, setHandoff] = useState<{ path?: string; voice?: string; n: number } | null>(null);
  // null = wird noch geprüft; ohne Tauri (reiner Browser) gilt alles als eingerichtet
  const [setup, setSetup] = useState<SetupStatus | null>(null);
  useEffect(() => {
    invoke<SetupStatus>("setup_status")
      .then(setSetup)
      .catch(() => setSetup({ core: true, music: true, folder: "" }));
  }, []);
  const needsSetup = setup !== null && !setup.core;
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
            disabled={!!a.phase || needsSetup}
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
        <UpdateBanner />
        {needsSetup ? (
          <>
            <h1>Willkommen</h1>
            <p className="muted">
              Gesang und Melodien lokal umwandeln, Stimmen trainieren und Musik erzeugen – ohne Konto, ohne Limit.
            </p>
            <SetupPanel part="core" onDone={setSetup} />
          </>
        ) : area === "convert" ? (
          <ConvertPage
            key={handoff?.n ?? 0}
            initialInput={handoff?.path ?? null}
            initialVoice={handoff?.voice ?? null}
            onOpenVoices={() => {
              setHandoff(null);
              setArea("voices");
            }}
          />
        ) : area === "separate" ? (
          <SeparatePage />
        ) : area === "clean" ? (
          <CleanPage />
        ) : area === "voices" ? (
          <VoicesPage
            onUse={(voice) => {
              setHandoff({ voice, n: (handoff?.n ?? 0) + 1 });
              setArea("convert");
            }}
          />
        ) : area === "history" ? (
          <HistoryPage
            onConvert={(path) => {
              setHandoff({ path, n: (handoff?.n ?? 0) + 1 });
              setArea("convert");
            }}
          />
        ) : area === "start" ? (
          <>
            <h1>Willkommen</h1>
            <p className="muted">
              Gesang und Melodien lokal umwandeln, Stimmen trainieren und Musik erzeugen – ohne Konto, ohne Limit.
            </p>
            <button onClick={() => setArea("convert")}>Stimme umwandeln</button>
            <EngineStatus />
          </>
        ) : null}
        {/* immer da, nur versteckt: Training und Lieder laufen weiter, während man andere Seiten nutzt */}
        {setup?.core && (
          <>
            <SongPage instrumental={false} active={area === "song"} musicReady={setup.music} onSetup={setSetup} />
            <SongPage instrumental active={area === "instrumental"} musicReady={setup.music} onSetup={setSetup} />
          </>
        )}
        <TrainPage
          active={area === "train"}
          onUse={(voice) => {
            setHandoff({ voice, n: (handoff?.n ?? 0) + 1 });
            setArea("convert");
          }}
        />
      </main>
    </div>
  );
}
